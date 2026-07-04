use thiserror::Error;
use std::ffi::OsString;

#[derive(Debug, Error)]
pub enum CmdlineReplacerError {
	#[error("Failed sending request to cmdline replacer: {0:#?}")]
	SendError(tokio::sync::mpsc::error::SendError<
		ReplacerCommand,
	>),

	#[error("Failed receiving data from cmdline replacer: {0:#?}")]
	ReceiveError(tokio::sync::oneshot::error::RecvError)
}

#[derive(Clone)]
pub struct Replacer {
	tx_query: tokio::sync::mpsc::Sender<ReplacerCommand>,
}

pub enum ReplacerCommand {
	Add {
		map: std::collections::HashMap<OsString, OsString>
	},
	// Remove {
	// 	origin: Vec<OsString>,
	// },
	Rewrite {
		original_args: Vec<OsString>,
		responder: tokio::sync::oneshot::Sender<Vec<OsString>>,
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
		map: std::collections::HashMap<OsString, OsString>,
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
		original_args: Vec<OsString>
	) -> Result<Vec<OsString>, CmdlineReplacerError> {
		let (tx, rx) = tokio::sync::oneshot::channel::<Vec<OsString>>();
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

	// pub async fn rm (
	// 	self: &Self,
	// 	origins: Vec<OsString>,
	// ) -> Result<(), CmdlineReplacerError> {
	// 	let result = self.tx_query.clone().send(
	// 		ReplacerCommand::Remove { origin: origins }
	// 	).await;
	// 	match result {
	// 		Ok(_)	=> Ok(()),
	// 		Err(e)	=> Err(CmdlineReplacerError::SendError(e))
	// 	}
	// }
}

async fn run(
	cancel_token:	tokio_util::sync::CancellationToken,
	mut rx_query:	tokio::sync::mpsc::Receiver<ReplacerCommand>,
) {
	let mut mappings =	std::collections::HashMap::<OsString, OsString>::new();
	loop {
		let cmd = tokio::select! {
			_ = cancel_token.cancelled()	=> {return}
			c = rx_query.recv()		=> {c}
		};

		let cmd = match cmd {
			Some(v)	=> v,
			None	=> {return}
		};

		match cmd {
			ReplacerCommand::Add { map } => {
				for (k, v) in map.iter() {
					let result = mappings.contains_key(k);
					match result {
						true	=> {
							mappings.remove(k);
						}
						false	=> {}
					};
					mappings.insert(k.into(), v.into());
				};
			}
			// ReplacerCommand::Remove { origin } => {
			// 	for val in origin.iter() {
			// 		mappings.remove(val);
			// 	};
			// }
			ReplacerCommand::Rewrite { original_args, responder } => {
				let mut resp: Vec<OsString> = vec![];
				for arg in original_args.iter() {
					if mappings.contains_key(arg) {
						// this should be safe since we check for existance
						resp.push(mappings.get(arg).unwrap().into());
					} else {
						resp.push(arg.to_owned());
					};
				};
				match responder.send(resp) {
					Ok(_)	=> {}
					Err(e)	=> {
						cancel_token.cancel();
						panic!("{e:#?}");
					}
				};
			}
		}
	}
}
