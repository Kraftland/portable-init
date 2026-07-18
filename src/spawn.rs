use thiserror::Error;

#[derive(Error, Debug)]
pub enum SpawnError {
	#[error("Could not send message via channel: {0:#?}")]
	ChannelSendError(tokio::sync::mpsc::error::SendError<crate::counter::CounterMessage>),

	#[error("Could not clone landlock rules: {0:#?}")]
	CloneLandlockError(std::io::Error),
}

#[derive(Clone)]
pub struct Spawner {
	tx:		tokio::sync::mpsc::Sender<SpawnMessage>,
}

#[derive(Debug)]
pub enum SpawnMessage {
	Start {
		target:	String,
		args:	Vec<String>,
		stream:	bool,
		reply:	Option<tokio::sync::oneshot::Sender<StartReply>>,
		envs: Option<std::collections::HashMap<String, String>>,
	}
}

#[derive(Debug)]
pub struct StartReply {
	// File descriptor for the slave pty
	pub master_fd: std::os::fd::OwnedFd,
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
		landlock_rules: landlock::RulesetCreated,
		seccomp_list: crate::seccomp::SyscallList,
	) -> Result<Self, SpawnError> {
		let (tx, rx) = tokio::sync::mpsc::channel::<SpawnMessage>(5);

		tokio::spawn(
			run(
				cancel_token,
				replacer,
				rx,
				counter,
				landlock_rules,
				seccomp_list,
				conf.clone(),
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
	landlock_rules:	landlock::RulesetCreated,
	seccomp_list:	crate::seccomp::SyscallList,
	conf:		crate::envs::ConfigOpts,
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
		let counter_tx = counter.send_channel.clone();

		let landlock_rules_clone = {
			match landlock_rules
				.try_clone()
				.map_err(SpawnError::CloneLandlockError) {
					Ok(v)	=> v,
					Err(e)	=> {
						crate::logger::log_fatal(
							format!("Could not clone landlock rules: {e:#?}"),
						);
						panic!("{e:#?}");
					}
				}
		};
		let seccomp_list = seccomp_list.clone();
		let conf_clone = conf.clone();

		tokio::spawn(async move {
			{
				if cancel_clone.is_cancelled() {
					return;
				}
			};

			{
				let filter = match crate::seccomp::compile_filter(
					&conf_clone,
					&seccomp_list,
				).await {
					Ok(v)	=> v,
					Err(e)	=> {
						crate::logger::log_fatal(
						 format!("Could not compile seccomp filter: {e:#?}")
						);
						panic!("Could not compile seccomp filter: {e:#?}")
					}
				};
				let fd = match crate::seccomp::load_seccomp_filter(
					filter,
				) {
					Ok(v)	=> {v}
					Err(e)	=> {
						crate::logger::log_fatal(
							format!("Could not load seccomp filter: {e:#?}"),
						);
						panic!("Could not load seccomp filter: {e:#?}");
					}
				};
				let cancel_clone = cancel_clone.clone();
				std::thread::spawn(
					 move || {
						crate::seccomp::process_seccomp_unotify(
							fd,
							cancel_clone.clone(),
						);
					}
				)
			};

			{
				if conf.lockdown {
					match crate::landlock::load_landlock(landlock_rules_clone) {
					Ok(_)	=> {
						crate::logger::log_debug("Loaded landlock rules".into());
					}
					Err(e)	=> {
						crate::logger::log_fatal(
							format!("Could not load landlock rules: {e:#?}"),
						);
					}
					};
				}
			};


			let msg = match msg {
				Some(v)	=>	v,
				None	=>	{return}
			};

			match msg {
				SpawnMessage::Start { target, args, stream, reply, envs } => {
					let args_new = replacer_clone.rewrite(args);
					let args_new = match args_new.await {
						Ok(v)	=> {v}
						Err(e)	=> {
							panic!("{e:#?}");
						}
					};

					let mut command = tokio::process::Command::new(target);
					let mut command = {
						match envs {
							Some(v)	=> {
								command.envs(v);
								command
							}
							None	=> {command}
						}
					};
					let command = command.args(args_new.iter());

					{
						let result = counter_tx.send(
							crate::counter::CounterMessage::ProcessStarted,
						)
							.await
							.map_err(SpawnError::ChannelSendError);
						match result {
							Ok(_)	=> {}
							Err(e)	=> {
								crate::logger::log_fatal(
									format!(
										"Could not contact counter: {e:#?}"
									)
								);
							}
						}
					};

					let command = if stream {

						let pty_pair = nix::pty::openpty(None, None)
							.unwrap();

						let master = pty_pair.master;


						let (stdin, stdout, stderr) = {
							let slave = pty_pair.slave;
							(slave.try_clone().unwrap(),
							slave.try_clone().unwrap(),
							slave)
						};



						command.stdin(stdin);
						command.stdout(stdout);
						command.stderr(stderr);

						// unwrap's safe because we should have channels on stream
						reply.unwrap().send(
							StartReply {
								//id: serial,
								master_fd: master,
							},
						).unwrap();
						command.kill_on_drop(true);

						command
					} else {
						command.kill_on_drop(true);
						command
					};

					crate::logger::log_debug(
						format!("Constructed command: {command:?}"),
					);

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
}
