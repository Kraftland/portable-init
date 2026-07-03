use std::ffi::OsString;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SpawnError {
	#[error("Could not create stream directory: {0:#?}")]
	MkStreamDirError(std::io::Error),

	#[error("Could not locate XDG_RUNTIME_DIR: {0:#?}")]
	RuntimeDirError(std::env::VarError),
}

pub struct Spawner {
	tx:		tokio::sync::mpsc::Sender<SpawnMessage>,
	base:		std::path::PathBuf,
	counter:	crate::counter::Counter,
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

		tokio::spawn(
			run(
				cancel_token,
				replacer,
				rx,
			),
		);

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

		Ok(Spawner {
			tx:		tx,
			base:		stream_path,
			counter:	counter,
		})
	}
}

async fn run(
	cancel_token: tokio_util::sync::CancellationToken,
	replacer: crate::process_env::Replacer,
	mut rx: tokio::sync::mpsc::Receiver<SpawnMessage>
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
		tokio::spawn(async move {
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



					if stream {
						// TODO: stream stuff here
					} else {

					};

				}
			}
		});

	}

	//std::fs::create_dir(path);
}

