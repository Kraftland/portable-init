mod logger;
use tokio::sync::mpsc;
mod seccomp;
mod envs;
mod landlock;
mod uclamp;

#[tokio::main]
async fn main() -> std::process::ExitCode {

	let (tx, rx) = mpsc::channel::<logger::LogMessage>(128);

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

	let tx_landlock_clone = tx.clone();
	let conf_clone = config_opts.clone();
	let landlock_spawn = tokio::spawn(
		async move {
			let result = uclamp::apply_uclamp();
			match result {
				Ok(val)	=> {
					logger::log(
						&tx_landlock_clone,
						logger::Loglevel::Warn,
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

	let _ = tokio::spawn(logger::logg_worker(rx));

	logger::log(&tx, logger::Loglevel::Debug, "Hello, World".to_string()).await;
	std::thread::sleep(std::time::Duration::from_secs(5));



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

	// TODO: start process

	return std::process::ExitCode::SUCCESS
}
