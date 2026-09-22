mod logger;
mod seccomp;
mod envs;
mod landlock;
mod uclamp;
mod spawn;
mod counter;
mod ipc;
mod process_env;
mod inhibit;
mod cleaner;

#[tokio::main]
async fn main() -> std::process::ExitCode {
	let cancel_token = tokio_util::sync::CancellationToken::new();

	let config_opts = {
		match envs::get().await {
			Ok(v)	=> v,
			Err(e)	=> {
				logger::log_fatal(
					format!("Could not obtain configurations via IPC: {e:#?}")
				);
				panic!("Could not obtain configurations via IPC: {e:#?}");
			}
		}
	};

	let replacer_spawn = {
		let cancel_token_clone = cancel_token.clone();
		tokio::spawn(process_env::Replacer::new(cancel_token_clone))
	};

	// #[cfg(debug_assertions)]
	// logger::log_debug(
	// 	format!("Got configurations: {config_opts:#?}"),
	// );

	let seccomp_spawn = tokio::spawn(
		seccomp::load(
			config_opts.clone(),
			cancel_token.clone(),
		),
	);

	let uclamp_ready = {
		let conf_clone = config_opts.clone();
		let cancel_token = tokio_util::sync::CancellationToken::new();
		let cancel_child = cancel_token.child_token();
		tokio::task::spawn(async move {
			match uclamp::apply_uclamp(
				conf_clone
			).await {
				Ok((min, max))	=> {
					#[cfg(debug_assertions)]
					logger::log_debug(
						format!("Successfully set uclamp.max to {min:?}:{max:?}"),
					);
				},
				Err(e)	=> {
					logger::log_warn(
						format!("Could not set uclamp: {e:#?}"),
					);
				}
			};
			cancel_token.cancel();
		});
		cancel_child
	};

	let landlock_result = {
		let conf_clone = config_opts.clone();
		tokio::spawn(async move {
			if ! conf_clone.landlock {
				return;
			}
			let rules = match landlock::compile_landlock_rules(&conf_clone).await {
				Ok(v)	=> v,
				Err(e)	=> {
					logger::log_fatal(
						format!("Could not compile landlock rules: {e:#?}"),
					);
					panic!("Could not compile landlock rules: {e:#?}")
				}
			};

			// Wait for uclamp completion, because landlock breaks that
			uclamp_ready.cancelled().await;

			landlock::load_landlock(rules)
				.await
				.expect("Could not load landlock rules");
		})
	};


	let counter_spawn = {
		let cancel_token_clone = cancel_token.clone();
		let bus_clone = config_opts.bus_conn.clone();

		tokio::spawn(
			async move {
				counter::Counter::new(
					cancel_token_clone,
					bus_clone,
				).await
			},
		)
	};

	let replacer = match replacer_spawn.await {
		Ok(v)	=> v,
		Err(e)	=> {
			logger::log_fatal(
				format!("Could not start cmdline replacer: {e:#?}"),
			);
			panic!("{e:#?}");
		}
	};

	let replacer = match replacer {
		Ok(v)	=> v,
		Err(e)	=> {
			logger::log_fatal(format!("Could not start cmdline replacer: {e:#?}"));
			panic!("{e:#?}");
		},
	};

	{
		let map = config_opts.file_map.clone();
		match replacer.add(map).await {
			Ok(_)	=> {}
			Err(e)	=> {
				logger::log_fatal(format!("Could not contact replacer: {e:#?}"));
				panic!("{e:#?}");
			}
		};
	}



	let counter = match counter_spawn.await {
		Ok(v)	=> v,
		Err(e)	=> {
			logger::log_fatal(format!("Could not contact replacer: {e:#?}"));
			panic!("{e:#?}");
		}
	};

	let spawner = {
		let replacer_clone = replacer.clone();
		let cancel_clone = cancel_token.clone();
		let spawner = spawn::Spawner::new(
			replacer_clone,
			cancel_clone,
			counter,
		);
		match spawner.await {
			Ok(v)	=> v,
			Err(e)	=> {
				logger::log_fatal(format!("Could not start task spawner: {e:#?}"));
				panic!("{e:#?}");
			},
		}
	};

	{
		seccomp_spawn
			.await
			.expect("Could not spawn seccomp thread")
			.expect("Could not load seccomp filter");

		landlock_result
			.await
			.expect("Could not load landlock rules");
	};

	spawner.spawn(
		spawn::SpawnMessage::Start {
			target: config_opts.target.to_string(),
			args: config_opts.args.clone(),
			stream: {
				match &config_opts.pty_fd {
					Some(v)	=> {
						spawn::StreamConsole::WithPty {
							fd:	v.try_clone().unwrap(),
						}
					}
					None	=> {
						spawn::StreamConsole::Direct
					}
				}
			},
			envs: None,
		}
	).await;

	let ipc_object = {
		let spawner_clone = spawner.clone();
		let conf_clone = config_opts.clone();
		let bus_publish_result = ipc::IPC::publish(
			conf_clone,
			replacer,
			spawner_clone,
		);


		match bus_publish_result.await {
			Ok(val)	=>	val,
			Err(e)	=>	{
				logger::log_fatal(format!("Could not connect to Session Bus: {e:#?}"));
				return std::process::ExitCode::FAILURE;
			}
		}
	};

	if config_opts.inhibit {
		let cancel_token_clone = cancel_token.clone();
		tokio::spawn(crate::inhibit::inhibit_suspend(cancel_token_clone));
	};

	let sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate());

	let mut sigterm = match sigterm {
		Ok(v)	=> {v}
		Err(e)	=> {
			panic!("Could not register signal listener: {e:#?}")
		}
	};

	tokio::select! {
		_ = cancel_token.cancelled()	=> {
			logger::log_info(format!("Shutting down on cancel token..."));
		},
		_ = tokio::signal::ctrl_c()	=> {
			logger::log_info(format!("Shutting down on SIGINT..."));
			cancel_token.cancel();
		},
		_ = sigterm.recv()
			=> {
			logger::log_info(format!("Shutting down on SIGTERM..."));
			cancel_token.cancel();
		}
	};

	match ipc_object.request_shutdown().await {
		Ok(_)	=> {}
		Err(e)	=> {
			crate::logger::log_warn(
				format!("Could not request IPC for shutdown: {e:#?}")
			);
		}
	};
	tokio::spawn(ipc_object.graceful_shutdown());

	return std::process::ExitCode::SUCCESS
}
