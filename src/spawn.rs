use std::ffi::OsString;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SpawnError {
	#[error("Could not create stream directory: {0:#?}")]
	MkStreamDirError(std::io::Error),

	#[error("Could not locate XDG_RUNTIME_DIR: {0:#?}")]
	RuntimeDirError(std::env::VarError),

	#[error("Could not bind on socket: {0:?}")]
	ListenStreamError(),
}

pub struct Spawner {
	tx:		tokio::sync::mpsc::Sender<SpawnMessage>,
}

enum SpawnMessage {
	Start {
		target:	OsString,
		args:	Vec<OsString>,
		stream:	bool,
		reply:	tokio::sync::oneshot::Receiver<StartReply>,
		envs: std::collections::HashMap<OsString, OsString>,
	}
}

struct StartReply {
	id:		usize,
	base_dir:	Option<std::path::PathBuf>,
}

impl Spawner {
	pub async fn new(
		conf: &crate::envs::ConfigOpts,
		replacer: crate::process_env::Replacer,
		cancel_token: tokio_util::sync::CancellationToken,
		counter: crate::counter::Counter,
	) -> Result<Self, SpawnError> {
		let (tx, rx) = tokio::sync::mpsc::channel::<SpawnMessage>(5);

		let runtime_dir = std::env::var("XDG_RUNTIME_DIR");
		let runtime_dir = match runtime_dir {
			Ok(v)	=> v,
			Err(e)	=> {
				return Err(
					SpawnError::RuntimeDirError(e)
				)
			}
		};

		let stream_path: std::path::PathBuf =
			[runtime_dir.as_str(), "portable", conf.sandbox_id.as_str(), "console"]
			.iter().
			collect();

		let result = std::fs::create_dir_all(&stream_path);
		match result {
			Ok(_)	=> {}
			Err(e)	=> {
				return Err(
					SpawnError::MkStreamDirError(e)
				);
			}
		}

		tokio::spawn(
			run(
				cancel_token,
				replacer,
				rx,
				counter,
				stream_path,
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
	base:		std::path::PathBuf,
) {

	let count_mu = std::sync::Arc::new(std::sync::Mutex::new(0));

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
		let mut counter_clone = std::sync::Arc::clone(&count_mu);
		tokio::spawn(async move {
			{
				let data = counter_clone.lock();
				match data {
					Ok(mut v)	=> {
						*v+=1;
					}
					Err(e)	=> {
						// TODO: failure handling
					}
				};
			};
			let msg = match msg {
				Some(v)	=>	v,
				None	=>	{return}
			};

			match msg {
				SpawnMessage::Start { target, args, stream, reply, envs } => {
					if cancel_clone.is_cancelled() {
						return;
					}
					let args_new = replacer_clone.rewrite(args);
					let args_new = match args_new.await {
						Ok(v)	=> {v}
						Err(_)	=> {
							// TODO: add logging here
							return;
						}
					};

					let mut command = std::process::Command::new(target);

					let command = command.envs(envs);
					let command = command.args(args_new.iter());

					counter_tx.send(
						crate::counter::CounterMessage::ProcessStarted,
					).await;

					if stream {
						// TODO: stream stuff here

					} else {

					};

					counter_tx.send(
						crate::counter::CounterMessage::ProcessDied,
					).await;

				}
			}
		});

	}

	//std::fs::create_dir(path);
}

enum SocketType {
	Stdin,
	Stdout,
	Stderr,
}

// Binds to the specific socket for streaming console
fn listen_socket_as_fd (
	base: &std::path::PathBuf,
	typ: SocketType,
) -> Result<std::os::fd::OwnedFd, SpawnError> {
	let sock_path = base.clone();
	match typ {
		SocketType::Stdin =>
	}
}
