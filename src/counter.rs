// use thiserror::Error;

pub struct Counter {
	pub send_channel: tokio::sync::mpsc::Sender<CounterMessage>,
}

pub enum CounterMessage {
	ProcessStarted,
	ProcessDied,
}

impl Counter {
	pub async fn new (
			cancel_token: tokio_util::sync::CancellationToken,
	) -> Self {
		let (tx, rx) = tokio::sync::mpsc::channel::<CounterMessage>(16);

		tokio::spawn(start(rx, cancel_token));

		Self { send_channel: tx }
	}
}

	async fn start (
			mut receive_chan: tokio::sync::mpsc::Receiver<CounterMessage>,
			cancel_token: tokio_util::sync::CancellationToken,
		) {
		let startup_result = systemd::daemon::notify(false, vec![("READY", "1")].iter());
		match startup_result {
			Ok(_)	=> {}
			Err(e)	=> {
				cancel_token.cancel();
				panic!("Could not set unit status: {e:#?}");
			}
		};
		let mut count: u128 = 0;
		loop {
			let msg = tokio::select! {
				val = receive_chan.recv() => {val}
				_ = cancel_token.cancelled() => {return}
			};
			let msg = match msg {
				Some(val)	=> val,
				None		=> continue,
			};
			match msg {
				CounterMessage::ProcessStarted	=> {
					count += 1;
				}
				CounterMessage::ProcessDied	=> {
					count -= 1;
				}
			}
			if count == 0 {
				cancel_token.cancel();

				let _ = systemd::daemon::notify(
					false,
					vec![(systemd::daemon::STATE_STOPPING, "1")].iter(),
				);
				return
			}
			let result = systemd::daemon::notify(
				false,
				vec![
					(
						systemd::daemon::STATE_STATUS,
						format!("Tracked PID count: {count}").as_str(),
					)
				].iter(),
			);
			match result {
				Ok(_)	=> {}
				Err(e)	=> {
					panic!("Could not set unit status: {e:#?}");
				}
			};
		}
	}
