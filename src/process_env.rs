use thiserror::Error;

#[derive(Debug, Error)]
pub enum CmdlineReplacerError {
	#[error("Failed sending request to cmdline replacer: {0:#?}")]
	SendError(tokio::sync::mpsc::error::SendError<
		ReplacerCommand,
	>),

	#[error("Failed receiving data from cmdline replacer: {0:#?}")]
	ReceiveError(tokio::sync::oneshot::error::RecvError)
}

pub struct Replacer {
	tx_query: tokio::sync::mpsc::Sender<ReplacerCommand>,
}

enum ReplacerCommand {
	Add {
		map: std::collections::HashMap<String, String>
	},
	Remove {
		origin: Vec<String>,
	},
	Rewrite {
		original_args: Vec<String>,
		responder: tokio::sync::oneshot::Sender<Vec<String>>,
	}
}

impl Replacer {
	pub async fn new(
		cancel_token: tokio_util::sync::CancellationToken,
	)
	-> Result<Self, CmdlineReplacerError> {
		let (tx_r, rx_r) = tokio::sync::mpsc::channel::<ReplacerCommand>(5);

		tokio::spawn(run(
			cancel_token,
			rx_r,
		));

		Ok(
			Self {
				tx_query: tx_r,
			},
		)
	}

	pub async fn add(
		self: &Self,
		map: std::collections::HashMap<String, String>,
	) -> Result<(), CmdlineReplacerError> {
		let cmd = ReplacerCommand::Add { map };
		let tx = self.tx_query.clone();
		match tx.send(cmd).await {
			Ok(_)	=> {Ok(())},
			Err(e)	=> {Err(CmdlineReplacerError::SendError(e))}
		}
	}

	pub async fn rewrite (
		self: &Self,
		original_args: Vec<String>
	) -> Result<Vec<String>, CmdlineReplacerError> {
		let (tx, rx) = tokio::sync::oneshot::channel::<Vec<String>>();
		let cmd = ReplacerCommand::Rewrite {
			original_args,
			responder: tx,
		};

		let tx_cmd = self.tx_query.clone();
		match tx_cmd.send(cmd).await {
			Ok(_)	=> {},
			Err(e)	=> {return Err(CmdlineReplacerError::SendError(e))}
		};

		let result = rx.await;
		match result {
			Ok(v)	=> Ok(v),
			Err(e)	=> Err(CmdlineReplacerError::ReceiveError(e))
		}
	}

	pub async fn rm (
		self: &Self,
		origins: Vec<String>,
	) -> Result<(), CmdlineReplacerError> {
		let result = self.tx_query.clone().send(
			ReplacerCommand::Remove { origin: origins }
		).await;
		match result {
			Ok(_)	=> Ok(()),
			Err(e)	=> Err(CmdlineReplacerError::SendError(e))
		}
	}
}

async fn run(
	cancel_token:	tokio_util::sync::CancellationToken,
	mut rx_query:	tokio::sync::mpsc::Receiver<ReplacerCommand>,
) {
	let mut mappings =	std::collections::HashMap::<String, String>::new();
	loop {
		let cmd = tokio::select! {
			_ = cancel_token.cancelled()	=> {return}
			c = rx_query.recv()		=> {c}
		};
	}
}
