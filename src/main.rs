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
	let task_tracker = tokio_util::task::TaskTracker::new();

	let cancel_token_clone = cancel_token.clone();
	let replacer_spawn = tokio::spawn(process_env::Replacer::new(cancel_token_clone));

	let seccomp_result = tokio::task::spawn_blocking(move || {
		match seccomp::compile_syscall_list() {
			Ok(v)	=> v,
			Err(e)	=> {
				logger::log_fatal(
					format!("Could not compile seccomp list: {e:#?}"),
				);
				panic!("Could not compile seccomp list: {e:#?}");
			}
		}
	});

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

	#[cfg(debug_assertions)]
	logger::log_debug(
		format!("Got configurations: {config_opts:#?}"),
	);


	let conf_clone = config_opts.clone();
	let uclamp_result = tokio::task::spawn(
		async {
			match uclamp::apply_uclamp(
				conf_clone
			).await {
				Ok((min, max))	=> {
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
		}
	);

	let conf_clone = config_opts.clone();
	let landlock_result = tokio::spawn(async move {
		if ! conf_clone.lockdown {
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

		landlock::load_landlock(rules)
			.await
			.expect("Could not load landlock rules");
	});

	let cancel_token_clone = cancel_token.clone();
	let counter_spawn = tokio::spawn(
		async move {
			return counter::Counter::new(
				cancel_token_clone,
			).await;
		},
	);

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


	let replacer_clone = replacer.clone();

	let counter = match counter_spawn.await {
		Ok(v)	=> v,
		Err(e)	=> {
			logger::log_fatal(format!("Could not contact replacer: {e:#?}"));
			panic!("{e:#?}");
		}
	};

	{
		let list = match seccomp_result.await {
			Ok(v)	=> {v}
			Err(e)	=> {
				logger::log_fatal(format!("Could not compile seccomp list: {e:#?}"));
				panic!("{e:#?}");
			}
		};

		let filter = match seccomp::compile_filter(
			config_opts.clone(),
			&list,
		).await {
			Ok(v)	=> v,
			Err(e)	=> {
				logger::log_fatal(format!("Could not compile seccomp filter: {e:#?}"));
				panic!("{e:#?}");
			}
		};
		let fd = match seccomp::load_seccomp_filter(filter) {
			Ok(v)	=> v,
			Err(e)	=> {
				logger::log_fatal(format!("Could load seccomp filter: {e:#?}"));
				panic!("{e:#?}");
			}
		};
		let cancel_token = cancel_token.clone();
		std::thread::spawn(move || {
			seccomp::process_seccomp_unotify(fd, cancel_token)
		})
	};

	landlock_result
		.await
		.expect("Could not load landlock rules");

	{
		match uclamp_result.await {
			Ok(_)	=> {}
			Err(e)	=> {
				logger::log_warn(format!("Could not spawn uclamp setter: {e:#?}"));
			}
		}
	};

	let spawner = {
		let cancel_clone = cancel_token.clone();
		let spawner = spawn::Spawner::new(
			replacer,
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

	let spawner_clone = spawner.clone();
	let conf_clone = config_opts.clone();
	let bus_publish_result = tokio::spawn(async move {
		let result = ipc::IPC::publish(
			conf_clone,
			replacer_clone,
			spawner_clone,
		).await;
		match result {
			Ok(val)	=> {
				logger::log_debug(format!("Connected to session bus"));
				val
			},
			Err(e)	=> {
				crate::logger::log_fatal(
					format!("Could not connect to session bus: {e:#?}"),
				);
				panic!("{e:#?}");
			},
		}
	});

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

	let ipc_object = match bus_publish_result.await {
		Ok(val)	=>	val,
		Err(e)	=>	{
			logger::log_fatal(format!("Could not connect to Session Bus: {e:#?}"));
			return std::process::ExitCode::FAILURE;
		}
	};

	task_tracker.close();

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

	task_tracker.wait().await;

	ipc_object.request_shutdown().await.unwrap();
	tokio::spawn(ipc_object.graceful_shutdown());

	return std::process::ExitCode::SUCCESS
}
