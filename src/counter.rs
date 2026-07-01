// use thiserror::Error;

pub struct Counter {
	send_channel: tokio::sync::mpsc::Sender<CounterMessage>,
	receive_channel: tokio::sync::mpsc::Receiver<CounterMessage>,
}

pub enum CounterMessage {
	ProcessStarted,
	ProcessDied,
}

impl Counter {
	pub fn new () -> Self {
		let (tx, mut rx) = tokio::sync::mpsc::channel::<CounterMessage>(16);
		Self { send_channel: tx , receive_channel: rx }
	}
	pub async fn start (
		mut receive_chan: tokio::sync::mpsc::Receiver<CounterMessage>,
		logtx: &tokio::sync::mpsc::Sender<crate::logger::LogMessage>,
		) {
		let startup_result = systemd::daemon::notify(false, vec![("READY", "1")].iter());
		match startup_result {
			Ok(_)	=> {}
			Err(e)	=> {
				crate::logger::log(
					&logtx,
					crate::logger::Loglevel::Warn,
					format!("Could not set unit status: {e:#?}")).await;
				return;
			}
		};
	}
}
