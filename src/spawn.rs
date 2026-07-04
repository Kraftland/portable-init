use std::ffi::OsString;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SpawnError {
	#[error("Could not create stream directory: {0:#?}")]
	MkStreamDirError(std::io::Error),

	#[error("Could not locate XDG_RUNTIME_DIR: {0:#?}")]
	RuntimeDirError(std::env::VarError),

	#[error("Could not bind on socket: {0:?}")]
	ListenStreamError(std::io::Error),
}

#[derive(Clone)]
pub struct Spawner {
	tx:		tokio::sync::mpsc::Sender<SpawnMessage>,
}

pub enum SpawnMessage {
	Start {
		target:	OsString,
		args:	Vec<OsString>,
		stream:	bool,
		reply:	tokio::sync::oneshot::Sender<StartReply>,
		envs: Option<std::collections::HashMap<OsString, OsString>>,
	}
}

#[derive(Debug)]
pub struct StartReply {
	pub base_dir:	Option<std::path::PathBuf>,
}

impl Spawner {
	pub async fn spawn (self: &Self, msg: SpawnMessage) {
		self.tx.send(msg).await.unwrap();
	}

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
		let counter_clone = std::sync::Arc::clone(&count_mu);
		let mut base_clone = base.clone();
		tokio::spawn(async move {
			{
				let data = counter_clone.lock();
				match data {
					Ok(mut v)	=> {
						*v+=1;
					}
					Err(e)	=> {
						panic!("{e:#?}")
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
						Err(e)	=> {
							panic!("{e:#?}");
						}
					};

					let mut command = tokio::process::Command::new(target);


					let command = {
						match envs {
							Some(v)	=> {command.envs(v)}
							None	=> {&mut command}
						}
					};
					let command = command.args(args_new.iter());

					counter_tx.send(
						crate::counter::CounterMessage::ProcessStarted,
					).await.unwrap();

					let command = if stream {
						let serial = {
							let count = counter_clone.lock().unwrap();
							*count
						}.to_string();

						base_clone.push(&serial);

						std::fs::create_dir_all(&base_clone).unwrap();

						let stdin = listen_socket_as_fd(
							&base_clone,
							SocketType::Stdin,
						).unwrap();

						let stdout = listen_socket_as_fd(
							&base_clone,
							SocketType::Stdout,
						).unwrap();

						let stderr = listen_socket_as_fd(
							&base_clone,
							SocketType::Stderr,
						).unwrap();

						command.stdin(stdin);
						command.stdout(stdout);
						command.stderr(stderr);

						reply.send(
							StartReply {
								//id: serial,
								base_dir: Some(base_clone),
							},
						).unwrap();

						command
					} else {
						command
					};

					let mut result = command
						.spawn()
						.unwrap();



					tokio::select! {
						_ = cancel_clone.cancelled() => {return}
						_ = result.wait() => {}
					};

					counter_tx.send(
						crate::counter::CounterMessage::ProcessDied,
					).await.unwrap();

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
	let mut sock_path = base.clone();
	let name = match typ {
		SocketType::Stdin	=> {"stdin"}
		SocketType::Stdout	=> {"stdout"}
		SocketType::Stderr	=> {"stderr"}
	};

	sock_path.push(name);

	let listener = std::os::unix::net::UnixListener::bind(sock_path);

	let listener = match listener {
		Ok(v)	=> v,
		Err(e)	=> {
			return Err(SpawnError::ListenStreamError(e));
		}
	};

	return Ok(std::os::fd::OwnedFd::from(listener))

}
