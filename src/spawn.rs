use thiserror::Error;

#[derive(Error, Debug)]
pub enum SpawnError {
	#[error("Could not send message via channel: {0:#?}")]
	ChannelSendError(tokio::sync::mpsc::error::SendError<crate::counter::CounterMessage>),
}

/**
	Designates the mode of console handling

	Direct means stream to Init's stdin/out/error, and is deprecated

	WithPty connects the processes' stdin/out/error to the pty
*/
#[derive(Debug)]
pub enum StreamConsole {
	Direct,
	WithPty { fd: std::os::fd::OwnedFd },
}

#[derive(Clone)]
pub struct Spawner {
	tx:		tokio::sync::mpsc::Sender<SpawnMessage>,
}

#[derive(Debug)]
pub enum SpawnMessage {
	Start {
		target:	String,
		args:	Vec<String>,
		stream:	StreamConsole,
		envs: Option<std::collections::HashMap<String, String>>,
	}
}

impl Spawner {
	pub async fn spawn (self: &Self, msg: SpawnMessage) {
		self.tx.send(msg).await.unwrap();
	}

	pub async fn new(
		replacer: crate::process_env::Replacer,
		cancel_token: tokio_util::sync::CancellationToken,
		counter: crate::counter::Counter,
	) -> Result<Self, SpawnError> {
		let (tx, rx) = tokio::sync::mpsc::channel::<SpawnMessage>(5);

		tokio::spawn(
			run(
				cancel_token,
				replacer,
				rx,
				counter,
			),
		);

		Ok(Spawner {
			tx:		tx,
		})
	}
}

async fn run(
	cancel_token:	tokio_util::sync::CancellationToken,
	replacer:	crate::process_env::Replacer,
	mut rx:		tokio::sync::mpsc::Receiver<SpawnMessage>,
	counter:	crate::counter::Counter,
) {
	loop {
		let msg = tokio::select! {
			_	= cancel_token.cancelled()	=> {
				return;
			}
			e	= rx.recv()			=> {
				e
			}
		};

		let cancel_clone = cancel_token.clone();
		let replacer_clone = replacer.clone();
		let counter_tx = counter.send_channel.clone();

		tokio::spawn(async move {
			{
				if cancel_clone.is_cancelled() {
					return;
				}
			};


			let msg = match msg {
				Some(v)	=>	v,
				None	=>	{return}
			};

			match msg {
				SpawnMessage::Start { target, args, stream, envs } => {
					let args_new = replacer_clone.rewrite(args);
					let args_new = match args_new.await {
						Ok(v)	=> {v}
						Err(e)	=> {
							panic!("{e:#?}");
						}
					};

					let mut command = tokio::process::Command::new(target);
					let mut command = {
						match envs {
							Some(v)	=> {
								command.envs(v);
								command
							}
							None	=> {command}
						}
					};

					let command = command.args(args_new.iter());

					{
						let result = counter_tx.send(
							crate::counter::CounterMessage::ProcessStarted,
						)
							.await
							.map_err(SpawnError::ChannelSendError);
						match result {
							Ok(_)	=> {}
							Err(e)	=> {
								crate::logger::log_fatal(
									format!(
										"Could not contact counter: {e:#?}"
									)
								);
							}
						}
					};

					let (command, _fd) = match stream {
						StreamConsole::Direct		=> {
							(command, None)
						}
						StreamConsole::WithPty { fd }	=> {
							nix::ioctl_none_bad!(
								tiocsctty,
								nix::libc::TIOCSCTTY
							);
							use std::os::fd::AsRawFd;
							let fd_raw = fd.as_raw_fd();
							unsafe {
								command.pre_exec(move || {
									nix::unistd::setsid()
									?;

									tiocsctty(fd_raw)
									?;
									nix::libc::dup2(
										fd_raw,
										nix::libc::STDIN_FILENO,
									);
									nix::libc::dup2(
										fd_raw,
										nix::libc::STDOUT_FILENO,
									);
									nix::libc::dup2(
										fd_raw,
										nix::libc::STDERR_FILENO,
									);

									#[cfg(debug_assertions)]
									println!("Begin terminal stream...");

									Ok(())
								});
							};
							(command, Some(fd))
						}
					};

					command.kill_on_drop(true);

					crate::logger::log_debug(
						format!("Constructed command: {command:?}"),
					);

					let mut result = command
						.spawn()
						.expect("Could not spawn command");

					let status = tokio::select! {
						_ = cancel_clone.cancelled() => {return}
						v = result.wait() => {v}
					};

					#[cfg(debug_assertions)]
					crate::logger::log_debug(
						format!("Child exited: {status:?}")
					);

					counter_tx.send(
						crate::counter::CounterMessage::ProcessDied,
					).await.unwrap();
				}
			}
		});
	}
}
