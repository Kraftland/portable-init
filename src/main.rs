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

	let config_opts = envs::get_configurations();
	let config_opts = match config_opts {
		Ok(conf) => conf,
		Err(e) => {
			logger::log(&tx, logger::Loglevel::Fatal, format!("{e}:?")).await;
			return std::process::ExitCode::FAILURE;
		}
	};

	let tx_clone_compile_syscall = tx.clone();
	let conf_clone = config_opts.clone();
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

		std::thread::spawn(
			move || {
				seccomp::process_seccomp_unotify(fd, &tx_clone);
			}
		);
	});

	let conf_clone = config_opts.clone();

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

	let spawner = {
		let cancel_clone = cancel_token.clone();
		let spawner = spawn::Spawner::new(&conf_clone, replacer, cancel_clone);
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

	let tx_clone = tx.clone();
	//let cancel_token_clone = cancel_token.clone();
	let bus_connect_result = tokio::spawn(async move {
		let tx_clone_2 = tx_clone.clone();
		let result = ipc::IPC::connect(
			&conf_clone,
			replacer_clone,
			tx_clone_2,
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

	let tx_landlock_clone = tx.clone();
	let conf_clone = config_opts.clone();
	let landlock_spawn = tokio::spawn(
		async move {
			let result = uclamp::apply_uclamp();
			match result {
				Ok(val)	=> {
					logger::log(
						&tx_landlock_clone,
						logger::Loglevel::Debug,
						format!("Successfully set uclamp.max to: {val:?}"),
					).await;
				}
				Err(e)	=> {
					logger::log(
						&tx_landlock_clone,
						logger::Loglevel::Warn,
						format!("Could not set uclamp limits: {e:#?}"),
					).await;
				}
			}

			let raw_env = std::env::var("_portableEnableLandlock");
			match raw_env {
				Ok(_)	=> {}
				Err(_)	=> {return}
			};
			let result = landlock::load_landlock(&conf_clone);
			match result {
				Ok(()) => {
					logger::log(
						&tx_landlock_clone,
						logger::Loglevel::Debug,
						format!("Loaded landlock rules"),
					).await;
				}
				Err(e) => {
					logger::log(
						&tx_landlock_clone,
						logger::Loglevel::Fatal,
						format!("Could not load landlock: {e:#?}"),
					).await;
				}
			}
		}
	);

	let counter_info = counter::Counter::new();
	let tx_clone = tx.clone();
	let cancel_token_clone = cancel_token.clone();
	let _ = tokio::spawn(
		async move {
			counter::Counter::start(
				counter_info.receive_channel,
				&tx_clone,
				cancel_token_clone,
			).await;
		},
	);

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
	match landlock_spawn.await {
		Ok(())	=> {}
		Err(e)	=> {
			logger::log(
				&tx,
				logger::Loglevel::Fatal,
				format!("Could not dispatch landlock thread: {e:#?}"),
			).await;
		}
	};

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

	println!("Starting process...");

	// TODO: start process

	task_tracker.close();

	tokio::select! {
		_ = cancel_token.cancelled()	=> {
			println!("Shutting down on cancel...");
		},
		_ = tokio::signal::ctrl_c()	=> {
			println!("Shutting down on SIGINT...");
			cancel_token.cancel();
		},
	};

	task_tracker.wait().await;

	ipc_object.request_shutdown().await.unwrap();
	tokio::spawn(ipc_object.graceful_shutdown());

	return std::process::ExitCode::SUCCESS
}
