use tokio;


pub async fn logg_worker(mut rx: tokio::sync::mpsc::Receiver<LogMessage>) {
	loop {
		let request = rx.recv().await.unwrap();
		match request.Level {
			LogLevel::Debug
				=> println!(
					"\x1b[38;2;125;241;118m[Init]\x1b[0m: {}",
					request.Message,
				),
			LogLevel::Info
				=> println!(
					"\x1b[38;2;119;222;250m[Init]\x1b[0m: {}",
					request.Message,
				),
			LogLevel::Warn
				=> println!(
					"\x1b[38;2;255;209;59m[Init]\x1b[0m: {}",
					request.Message,
				),
			LogLevel::Fatal
				=> println!(
					"\x1b[38;2;255;0;0m[Init]\x1b[0m: {}",
					request.Message,
				)
		}
	}
}

#[derive(Debug)]
pub struct LogMessage {
	Level:		LogLevel,
	Message:	String,
}

#[derive(Debug)]
pub enum LogLevel {
	Debug,
	Info,
	Warn,
	Fatal,
}

pub async fn log_internal(tx: tokio::sync::mpsc::Sender<LogMessage>, level: LogLevel, msg: String) {
	tx.send(LogMessage { Level: level, Message: msg }).await.ok();
}

pub fn log(tx: &tokio::sync::mpsc::Sender<LogMessage>, level: LogLevel, msg: String) {
	let tx_new = tx.clone();
	tokio::spawn(async {
		log_internal(tx_new, level, msg).await;
	});
}
