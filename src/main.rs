mod logger;
use tokio::sync::mpsc;
mod seccomp;
mod envs;

#[tokio::main]
async fn main() -> std::process::ExitCode {

	let (tx, rx) = mpsc::channel::<logger::LogMessage>(16);

	let config_opts = envs::get_configurations();
	let config_opts = match config_opts {
		Ok(conf) => conf,
		Err(e) => {
			logger::log(&tx, logger::Loglevel::Fatal, format!("{e}:?"));
			return std::process::ExitCode::FAILURE;
		}
	};

	let tx_clone_compile_syscall = tx.clone();
	let seccomp_result = tokio::spawn(async move {
		let result = seccomp::compile_syscall_list(&tx_clone_compile_syscall);
		let list = match result {
			Ok(val)	=> val,
			Err(e)	=> {
				logger::log(
					&tx_clone_compile_syscall,
					logger::Loglevel::Fatal,
					format!("{e}"),
				);
				return
			}
		};
		let result = seccomp::load_seccomp_filter(&config_opts, &list);
		let fd = match result {
			Ok(fd) => fd,
			Err(e) => {
				logger::log(
					&tx_clone_compile_syscall,
					logger::Loglevel::Fatal,
					format!("{e:#?}"),
				);
				return
			}
		};
		logger::log(
			&tx_clone_compile_syscall,
			logger::Loglevel::Info,
			"Loaded seccomp filter".into(),
		);
	});

	let _ = tokio::spawn(logger::logg_worker(rx));

	logger::log(&tx, logger::Loglevel::Debug, "Hello, World".to_string());
	std::thread::sleep(std::time::Duration::from_secs(5));



	match seccomp_result.await {
		Ok(())	=> {}
		Err(e)	=> {
			logger::log(
				&tx,
				logger::Loglevel::Fatal,
				format!("Could not dispatch seccomp thread: {e:#?}"),
			);
		}
	};

	// TODO: start process

	return std::process::ExitCode::SUCCESS
}
