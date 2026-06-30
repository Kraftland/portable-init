use tokio;

#[cfg(debug_assertions)]
fn get_default_level() -> Loglevel {
	Loglevel::Info
}

#[cfg(not(debug_assertions))]
fn get_default_level() -> Loglevel {
	Loglevel::Debug
}

pub async fn logg_worker(mut rx: tokio::sync::mpsc::Receiver<LogMessage>) {
	let mut log_level = get_default_level();
	let raw_os_level = std::env::var("PORTABLE_LOGGING");
	match raw_os_level {
		Ok(val)	=> {
			if val == "debug" {
				log_level = Loglevel::Debug;
			}
		}
		Err(_)	=> {}
	}


	loop {
		let request = rx.recv().await.unwrap();
		match request.level {
			Loglevel::Debug
				=> {
					match log_level {
						Loglevel::Debug	=> continue,
						_		=> {}
					};
					println!(
						"\x1b[38;2;125;241;118m[Init]\x1b[0m: {}",
						request.message,
					)
				}

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
				=> {
					println!(
						"\x1b[38;2;255;0;0m[Init]\x1b[0m: {}",
						request.message,
					);
					std::process::exit(1)
				}
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

pub async fn log(tx: &tokio::sync::mpsc::Sender<LogMessage>, level: Loglevel, msg: String) {
	let tx_new = tx.clone();
	tokio::spawn(async move {
		tx_new.send(LogMessage{
			level:		level,
			message:	msg,
		}).await
	});
}

pub fn log_sync(tx: &tokio::sync::mpsc::Sender<LogMessage>, level: Loglevel, msg: String) {
	tx.try_send(LogMessage{
		level:		level,
		message:	msg
	}).unwrap();
}
