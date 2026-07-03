use thiserror::Error;

// Responder for command line rewrite service, Type must be vec.
type Responder = tokio::sync::oneshot::Sender<Result<Vec<String>, ProcessEnvError>>;

#[derive(Debug, Error)]
pub enum ProcessEnvError {

}

pub struct Replacer {
	current_mappings: std::collections::HashMap<String, String>,
	tx_add: tokio::sync::mpsc::Sender<std::collections::HashMap<String, String>>,
	rx_add: tokio::sync::mpsc::Receiver<std::collections::HashMap<String, String>>,

	tx_rm: tokio::sync::mpsc::Sender<Vec<String>>,
	rx_rm: tokio::sync::mpsc::Receiver<Vec<String>>,
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
	pub fn new() -> Result<Self, ProcessEnvError> {
		let (tx_ad, rx_ad) = tokio::sync::mpsc::channel(5);
		let (tx_r, rx_r) = tokio::sync::mpsc::channel(5);
		Ok(
			Self {
				current_mappings: std::collections::HashMap::new(),
				tx_add: tx_ad,
				tx_rm: tx_r,
				rx_add: rx_ad,
				rx_rm: rx_r,
			},
		)
	}

	pub async fn run(self: &Self, cancel_token: tokio_util::sync::CancellationToken) {
		tokio::select! {
			_ = cancel_token.cancelled() => {return}
		}
	}

	pub async fn query(self: &Self, cmd: ReplacerCommand) -> Result<(), ProcessEnvError> {
		match cmd {
			ReplacerCommand::Add { map } => {
				let tx = self.tx_add.clone();
				tx.send(
					map
				).await;
			}
			ReplacerCommand::Remove { origin } => {

			}
			ReplacerCommand::Rewrite { original_args, responder } => {}
		};
	}
}
