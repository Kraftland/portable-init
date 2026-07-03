use thiserror::Error;

// Responder for command line rewrite service, Type must be vec.
type Responder = tokio::sync::oneshot::Sender<Result<Vec<String>, ProcessEnvError>>;

#[derive(Debug, Error)]
pub enum ProcessEnvError {
	#[error("Failed sending request to cmdline replacer: {0:#?}")]
	SendError(tokio::sync::mpsc::error::SendError<
		std::collections::HashMap<String, String>,
	>)
}

pub struct Replacer {
	tx_query: tokio::sync::mpsc::Sender<ReplacerCommand>,
}

pub enum ReplacerCommand {
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
	-> Result<Self, ProcessEnvError> {
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

	pub async fn query(self: &Self, cmd: ReplacerCommand) -> Result<(), ProcessEnvError> {
		match cmd {
			ReplacerCommand::Add { map } => {
				let tx = self.tx_add.clone();
				let result = tx.send(
					map
				).await;
				match result {
					Ok(_)	=> {Ok(())}
					Err(e)	=> {
						Err(
							ProcessEnvError::SendError(e)
						)
					}
				}
			}
			ReplacerCommand::Remove { origin } => {
				Ok(())
			}
			ReplacerCommand::Rewrite { original_args, responder } => {
				Ok(())
			}
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
