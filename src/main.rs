mod logger;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
	let (tx, rx) = mpsc::channel::<logger::LogMessage>(16);

	let _ = tokio::spawn(logger::logg_worker(rx));


	logger::log(&tx, logger::LogLevel::Debug, "Hello, World".to_string());
	std::thread::sleep(std::time::Duration::from_secs(5));
}
