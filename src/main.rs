mod logger;
use tokio::sync::mpsc;
mod seccomp;
mod envs;
mod landlock;
mod uclamp;
mod spawn;
mod counter;
mod ipc;
mod process_env;

#[tokio::main]
async fn main() -> std::process::ExitCode {
	let (tx, rx) = mpsc::channel::<logger::LogMessage>(128);
	let cancel_token = tokio_util::sync::CancellationToken::new();
	let task_tracker = tokio_util::task::TaskTracker::new();

	let cancel_token_clone = cancel_token.clone();
	let replacer_spawn = tokio::spawn(process_env::Replacer::new(cancel_token_clone));

	let cancel_token_clone = cancel_token.clone();
	let _ = tokio::spawn(logger::logg_worker(rx, cancel_token_clone));

	let config_opts = {
		match envs::get_configurations() {
			Ok(conf) => conf,
			Err(e) => {
				logger::log(&tx, logger::Loglevel::Fatal, format!("{e}:?")).await;
				return std::process::ExitCode::FAILURE;
			}
		}
	};

	let tx_clone = tx.clone();
	let uclamp_result = tokio::spawn(
		async move {
			match uclamp::apply_uclamp() {
				Ok(v)	=> {
					logger::log(
						&tx_clone,
						logger::Loglevel::Debug,
						format!("Successfully set uclamp.min to {v:?}"),
					).await;
				},
				Err(e)	=> {
					logger::log(
						&tx_clone,
						logger::Loglevel::Warn,
						format!("Could not compile landlock rules: {e:#?}"),
					).await;
					std::thread::sleep(std::time::Duration::from_secs(5));
					panic!("Could not compile landlock rules: {e:#?}")
				}
			};
		}
	);

	let conf_clone = config_opts.clone();
	let tx_clone = tx.clone();
	let landlock_result = tokio::spawn(async move {
		match landlock::compile_landlock_rules(&conf_clone).await {
			Ok(v)	=> v,
			Err(e)	=> {
				logger::log(
					&tx_clone,
					logger::Loglevel::Fatal,
					format!("Could not compile landlock rules: {e:#?}"),
				).await;
				std::thread::sleep(std::time::Duration::from_secs(5));
				panic!("Could not compile landlock rules: {e:#?}")
			}
		}
	});

	let tx_clone_compile_syscall = tx.clone();
	let conf_clone = config_opts.clone();

	let cancel_clone = cancel_token.clone();
	let seccomp_result = tokio::spawn(async move {
		let result = seccomp::compile_syscall_list(&tx_clone_compile_syscall);
		let list = match result {
			Ok(val)	=> val,
			Err(e)	=> {
				logger::log(
					&tx_clone_compile_syscall,
					logger::Loglevel::Fatal,
					format!("{e}"),
				).await;
				return
			}
		};
		let result = seccomp::load_seccomp_filter(&conf_clone, &list);
		let fd = match result {
			Ok(fd) => fd,
			Err(e) => {
				logger::log(
					&tx_clone_compile_syscall,
					logger::Loglevel::Fatal,
					format!("{e:#?}"),
				).await;
				return
			}
		};
		logger::log(
			&tx_clone_compile_syscall,
			logger::Loglevel::Info,
			"Loaded seccomp filter".into(),
		).await;

		let tx_clone = tx_clone_compile_syscall.clone();

		tokio::spawn(seccomp::process_seccomp_unotify(fd, tx_clone, cancel_clone));
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
			logger::log(
				&tx,
				logger::Loglevel::Fatal,
				format!("Could not start cmdline replacer: {e:#?}"),
			).await;
			std::thread::sleep(std::time::Duration::from_secs(5));
			panic!("{e:#?}");
		}
	};

	let replacer = match replacer {
		Ok(v)	=> v,
		Err(e)	=> {
			logger::log(
				&tx,
				logger::Loglevel::Fatal,
				format!("Could not start cmdline replacer: {e:#?}"),
			).await;
			std::thread::sleep(std::time::Duration::from_secs(5));
			panic!("{e:#?}");
		},
	};

	{
		let map = config_opts.file_map.clone();
		match replacer.add(map).await {
			Ok(_)	=> {}
			Err(e)	=> {
				logger::log(
					&tx,
					logger::Loglevel::Fatal,
					format!("Could not contact replacer: {e:#?}"),
				).await;
				std::thread::sleep(std::time::Duration::from_secs(5));
				panic!("{e:#?}");
			}
		};
	}


	let replacer_clone = replacer.clone();

	let counter = match counter_spawn.await {
		Ok(v)	=> v,
		Err(e)	=> {
			logger::log(
				&tx,
				logger::Loglevel::Fatal,
				format!("Could not contact replacer: {e:#?}"),
			).await;
			std::thread::sleep(std::time::Duration::from_secs(5));
			panic!("{e:#?}");
		}
	};

	match seccomp_result.await {
		Ok(())	=> {}
		Err(e)	=> {
			logger::log(
				&tx,
				logger::Loglevel::Fatal,
				format!("Could not dispatch seccomp thread: {e:#?}"),
			).await;
		}
	};

	let landlock_rules = landlock_result.await.unwrap();

	{
		match uclamp_result.await {
			Ok(_)	=> {}
			Err(e)	=> {
				logger::log(
					&tx,
					logger::Loglevel::Warn,
					format!("Could not spawn uclamp setter: {e:#?}"),
				).await;
			}
		}
	};

	let tx_clone = tx.clone();
	let conf_clone = config_opts.clone();
	let spawner = {
		let cancel_clone = cancel_token.clone();
		let spawner = spawn::Spawner::new(
			&conf_clone,
			replacer,
			cancel_clone,
			counter,
			tx_clone,
			landlock_rules,
		);
		match spawner.await {
			Ok(v)	=> v,
			Err(e)	=> {
				logger::log(
					&tx,
					logger::Loglevel::Fatal,
					format!("Could not start task spawner: {e:#?}"),
				).await;
				std::thread::sleep(std::time::Duration::from_secs(5));
				panic!("{e:#?}");
			},
		}
	};

	let spawner_clone = spawner.clone();
	let conf_clone = config_opts.clone();
	let tx_clone = tx.clone();
	let bus_connect_result = tokio::spawn(async move {
		let tx_clone_2 = tx_clone.clone();
		let result = ipc::IPC::connect(
			&conf_clone,
			replacer_clone,
			tx_clone_2,
			spawner_clone,
		).await;
		match result {
			Ok(val)	=> {
				logger::log(
					&tx_clone,
					logger::Loglevel::Debug,
					format!("Connected to session bus"),
				).await;
				val
			},
			Err(e)	=> {
				crate::logger::log(
					&tx_clone,
					crate::logger::Loglevel::Fatal,
					format!("Could not connect to session bus: {e:#?}"),
				).await;
				std::thread::sleep(std::time::Duration::from_secs(5));
				panic!("{e:#?}");
			},
		}
	});

	let ipc_object = match bus_connect_result.await {
		Ok(val)	=>	val,
		Err(e)	=>	{
			logger::log(
				&tx,
				logger::Loglevel::Fatal,
				format!("Could not connect to Session Bus: {e:#?}")
			).await;
			std::thread::sleep(std::time::Duration::from_secs(5));
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

	let sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate());

	let mut sigterm = match sigterm {
		Ok(v)	=> {v}
		Err(e)	=> {
			panic!("Could not register signal listener: {e:#?}")
		}
	};

	tokio::select! {
		_ = cancel_token.cancelled()	=> {
			println!("Shutting down on cancel...");
		},
		_ = tokio::signal::ctrl_c()	=> {
			println!("Shutting down on SIGINT...");
			cancel_token.cancel();
		},
		_ = sigterm.recv()
			=> {
			println!("Shutting down on SIGTERM (polite quit request)");
			cancel_token.cancel();
		}
	};

	task_tracker.wait().await;

	ipc_object.request_shutdown().await.unwrap();
	tokio::spawn(ipc_object.graceful_shutdown());

	return std::process::ExitCode::SUCCESS
}
