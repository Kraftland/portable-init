use thiserror::Error;

// Responder for command line rewrite service, Type must be vec.
type Responder = tokio::sync::oneshot::Sender<Result<Vec<String>, CmdlineReplacerError>>;

#[derive(Debug, Error)]
pub enum CmdlineReplacerError {
	#[error("Failed sending request to cmdline replacer: {0:#?}")]
	SendError(tokio::sync::mpsc::error::SendError<
		ReplacerCommand,
	>)
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
		responder: Responder,
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
}

async fn run(
	cancel_token:	tokio_util::sync::CancellationToken,
	mut rx_query:	tokio::sync::mpsc::Receiver<ReplacerCommand>,
) {
	let mut mappings =	std::collections::HashMap::<String, String>::new();
	loop {
		tokio::select! {
			_ = cancel_token.cancelled()	=> {return}
			c = rx_query.recv()		=> {}
		}
	}
}
