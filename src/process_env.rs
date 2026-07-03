use thiserror::Error;

// Responder for command line rewrite service, Type must be vec.
type Responder = tokio::sync::oneshot::Sender<Result<Vec<String>, ProcessEnvError>>;

#[derive(Debug, Error)]
pub enum ProcessEnvError {

}

pub struct Replacer;

pub enum ReplacerCommand {
	Add {
		origin: String,
		dest: String,
	},
	Remove {
		origin: String,
	},
	Rewrite {
		original_args: Vec<String>,
		responder: Responder,
	}
}

impl Replacer {
	pub fn new() -> Result<Self, ProcessEnvError> {
		Ok(
			Self {},
		)
	}
}
