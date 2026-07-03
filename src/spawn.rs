use std::ffi::OsString;

pub struct Spawner {
	tx:	tokio::sync::mpsc::Sender<SpawnMessage>,
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
	pub async fn new(cancel_token: tokio_util::sync::CancellationToken) -> Self {
		let (tx, rx) = tokio::sync::mpsc::channel::<SpawnMessage>(5);

		tokio::spawn(
			run(
				cancel_token,
				rx
			),
		);

		Spawner {
			tx: tx,
		}
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
}
