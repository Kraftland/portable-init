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


	let config_opts = {
		match envs::get_configurations() {
			Ok(conf) => conf,
			Err(e) => {
				logger::log_fatal(format!("{e}:?"));
				return std::process::ExitCode::FAILURE;
			}
		}
	};

	logger::log_debug(
		format!("Got configurations: {config_opts:#?}"),
	);

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

	let uclamp_result = tokio::task::spawn_blocking(
		move || {
			match uclamp::apply_uclamp() {
				Ok(v)	=> {
					logger::log_debug(
						format!("Successfully set uclamp.max to {v:?}"),
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
		match landlock::compile_landlock_rules(&conf_clone).await {
			Ok(v)	=> v,
			Err(e)	=> {
				logger::log_fatal(
					format!("Could not compile landlock rules: {e:#?}"),
				);
				panic!("Could not compile landlock rules: {e:#?}")
			}
		}
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

	let landlock_rules = landlock_result.await.unwrap();

	{
		match uclamp_result.await {
			Ok(_)	=> {}
			Err(e)	=> {
				logger::log_warn(format!("Could not spawn uclamp setter: {e:#?}"));
			}
		}
	};

	let seccomp_list = {
		match seccomp_result.await {
			Ok(v)	=> {v}
			Err(e)	=> {
				logger::log_fatal(format!("Could not compile seccomp list: {e:#?}"));
				panic!("{e:#?}");
			}
		}
	};

	let conf_clone = config_opts.clone();
	let spawner = {
		let cancel_clone = cancel_token.clone();
		let spawner = spawn::Spawner::new(
			&conf_clone,
			replacer,
			cancel_clone,
			counter,
			landlock_rules,
			seccomp_list,
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
	let bus_connect_result = tokio::spawn(async move {
		let result = ipc::IPC::connect(
			&conf_clone,
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

	let ipc_object = match bus_connect_result.await {
		Ok(val)	=>	val,
		Err(e)	=>	{
			logger::log_fatal(format!("Could not connect to Session Bus: {e:#?}"));
			return std::process::ExitCode::FAILURE;
		}
	};

	spawner.spawn(
		spawn::SpawnMessage::Start {
			target: config_opts.target,
			args: config_opts.args,
			stream: false,
			reply: None,
			envs: None,
		}
	).await;

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
