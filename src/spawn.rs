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
	tx:	tokio::sync::mpsc::Sender<SpawnMessage>,
	base:	std::path::PathBuf,
}

enum SpawnMessage {
	Start {
		target:	OsString,
		args:	Vec<OsString>,
		reply:	tokio::sync::oneshot::Receiver<StartReply>,
	}
}

struct StartReply {
	id:		usize,
	stream:		bool,
	base_dir:	std::path::PathBuf,
}

impl Spawner {
	pub async fn new(
		conf: &crate::envs::ConfigOpts,
		cancel_token: tokio_util::sync::CancellationToken,
	) -> Result<Self, SpawnError> {
		let (tx, rx) = tokio::sync::mpsc::channel::<SpawnMessage>(5);

		tokio::spawn(
			run(
				cancel_token,
				rx
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
			tx:	tx,
			base:	stream_path,
		})
	}
}

async fn run(
	cancel_token: tokio_util::sync::CancellationToken,
	mut rx: tokio::sync::mpsc::Receiver<SpawnMessage>
) {
	let msg = tokio::select! {
		_	= cancel_token.cancelled()	=> {return}
		e	= rx.recv()			=> {
			e
		}
	};
	let msg = match msg {
		Some(v)	=> v,
		None	=> return,
	};

	//std::fs::create_dir(path);
}
