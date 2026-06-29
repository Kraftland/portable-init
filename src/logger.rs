use tokio;


pub async fn logg_worker(mut rx: tokio::sync::mpsc::Receiver<LogMessage>) {
	loop {
		let request = rx.recv().await.unwrap();
		match request.level {
			Loglevel::Debug
				=> println!(
					"\x1b[38;2;125;241;118m[Init]\x1b[0m: {}",
					request.message,
				),
			Loglevel::Info
				=> println!(
					"\x1b[38;2;119;222;250m[Init]\x1b[0m: {}",
					request.message,
				),
			Loglevel::Warn
				=> println!(
					"\x1b[38;2;255;209;59m[Init]\x1b[0m: {}",
					request.message,
				),
			Loglevel::Fatal
				=> println!(
					"\x1b[38;2;255;0;0m[Init]\x1b[0m: {}",
					request.message,
				)
		}
	}
}

#[derive(Debug)]
pub struct LogMessage {
	level:		Loglevel,
	message:	String,
}

#[derive(Debug)]
pub enum Loglevel {
	Debug,
	Info,
	Warn,
	Fatal,
}

pub async fn log_internal(tx: tokio::sync::mpsc::Sender<LogMessage>, level: Loglevel, msg: String) {
	tx.send(LogMessage { level: level, message: msg }).await.ok();
}

pub fn log(tx: &tokio::sync::mpsc::Sender<LogMessage>, level: Loglevel, msg: String) {
	let tx_new = tx.clone();
	tokio::spawn(async {
		log_internal(tx_new, level, msg).await;
	});
}
