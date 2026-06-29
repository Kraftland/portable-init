mod logger;
use tokio::sync::mpsc;
mod seccomp;
mod envs;

#[tokio::main]
async fn main() -> std::process::ExitCode {
	let (tx, rx) = mpsc::channel::<logger::LogMessage>(16);

	let _ = tokio::spawn(logger::logg_worker(rx));


	let config_opts = envs::get_configurations();
	let config_opts = match config_opts {
		Ok(conf) => conf,
		Err(e) => {
			logger::log(&tx, logger::Loglevel::Fatal, format!("{e}:?"));
			return std::process::ExitCode::FAILURE;
		}
	};

	logger::log(&tx, logger::Loglevel::Debug, "Hello, World".to_string());
	std::thread::sleep(std::time::Duration::from_secs(5));


	return std::process::ExitCode::SUCCESS
}
