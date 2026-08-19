// use thiserror::Error;

mod status;

pub struct Counter {
	pub	send_channel: tokio::sync::mpsc::Sender<CounterMessage>,
}

pub enum CounterMessage {
	ProcessStarted,
	ProcessDied,
}

impl Counter {
	pub async fn new (
			cancel_token:	tokio_util::sync::CancellationToken,
			bus:		zbus::Connection,
	) -> Self {
		let (tx, rx) = tokio::sync::mpsc::channel::<CounterMessage>(16);

		tokio::spawn(start(rx, cancel_token, bus));

		Self { send_channel: tx }
	}
}

	async fn start (
			mut receive_chan:	tokio::sync::mpsc::Receiver<CounterMessage>,
			cancel_token:		tokio_util::sync::CancellationToken,
			bus:			zbus::Connection,
		) {

		let (systemd_notify, portal_notify) = {
			use status::Init;

			let sd = status::systemd::SystemdStatus {};

			match sd.initialise().await {
				Ok(v)	=> {v}
				Err(e)	=> {
					crate::logger::log_warn(
						format!("Could not initialise systemd status: {e:#?}")
					);
				}
			};

			let portal = status::portal::PortalStatus {
				bus:	bus,
			};

			(std::sync::Arc::new(sd), std::sync::Arc::new(portal))
		};

		let mut count: usize = 0;
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

			match count {
				0	=> {
					use status::UpdateStatus;

					let stat = status::SandboxStatus::Stopping;

					match systemd_notify.update(&stat).await {
						Ok(_)	=> {}
						Err(e)	=> {
							crate::logger::log_warn(
								format!(
								"Could not update systemd status: {e:#?}",
								)
							);
						}
					};

					match portal_notify.update(&stat).await {
						Ok(_)	=> {}
						Err(e)	=> {
							crate::logger::log_warn(
								format!(
								"Could not update Background status: {e:#?}",
								)
							);
						}
					};

					cancel_token.cancel();

					return;
				}
				v	=> {
					use status::UpdateStatus;

					let stat = status::SandboxStatus::Ready { tracked_pid: v };

					match systemd_notify.update(&stat).await {
						Ok(_)	=> {}
						Err(e)	=> {
							crate::logger::log_warn(
								format!(
								"Could not update systemd status: {e:#?}",
								)
							);
						}
					};

					match portal_notify.update(&stat).await {
						Ok(_)	=> {}
						Err(e)	=> {
							crate::logger::log_warn(
								format!(
								"Could not update Background status: {e:#?}",
								)
							);
						}
					};
				}
			};
		}
	}
