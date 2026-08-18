use thiserror::Error;

mod tray;

struct Init {
	replacer:	crate::process_env::Replacer,
	spawner:	crate::spawn::Spawner,
	conf:		std::sync::Arc<crate::envs::ConfigOpts>,
}

#[zbus::interface(
	name = "top.kimiblock.Portable.Init",
	introspection_docs = true,
)]
impl Init {
	#[zbus(name = "ActivateTray")]
	async fn activate_tray(&self) -> zbus::fdo::Result<()> {
		tray::wake().await
	}

	#[zbus(
		name		= "AuxStart3Silent",
	)]
	async fn start_without_pty(
		&self,
		custom_target:	bool,
		target_exec:	String,
		args_append:	bool,
		arguments:	Vec<String>,
		extra_files:	std::collections::HashMap<String, String>,
		envs:		std::collections::HashMap<String, String>,
	) -> zbus::fdo::Result<()> {
		#[cfg(debug_assertions)]
		{
			let mut log_msg = String::from("Got start request from D-Bus: ");
			log_msg.push_str(format!("Custom target: {custom_target}; ").as_str());
			log_msg.push_str(format!("target: {target_exec}; ").as_str());
			log_msg.push_str(format!("append arguments: {args_append}; ").as_str());
			log_msg.push_str(format!("arguments: {arguments:?}; ").as_str());
			log_msg.push_str(format!("extra files: {extra_files:?}; ").as_str());
			log_msg.push_str(format!("variables: {envs:?}; ").as_str());
			crate::logger::log_debug(log_msg);
		};



		let mut args: Vec<String> = vec![];

		if extra_files.len() > 0 {
			match self.replacer.add(extra_files).await {
				Ok(_)	=> {}
				Err(e)	=> {
					return Err(zbus::fdo::Error::Failed(format!("{e:#?}")))
				}
			};
		};


		let target: String = {
			if custom_target {
				target_exec.into()
			} else {
				self.conf.target.clone()
			}
		};


		if args_append {
			for val in self.conf.args.iter() {
				args.push(val.clone());
			};
		}

		args.extend(arguments);

		self.spawner.spawn(
			crate::spawn::SpawnMessage::Start {
				target: target,
				args: args,
				stream: crate::spawn::StreamConsole::Direct,
				// stream: crate::spawn::StreamConsole::Direct,
				envs: {
					if envs.len() > 0 {
						Some(envs)
					} else {
						None
					}
				},
			}
		).await;

		#[cfg(debug_assertions)]
		crate::logger::log_debug(format!("Sent spawn instructions"));

		Ok(())
	}

	#[zbus(
		name		= "AuxStart3",
	)]
	async fn start_with_pty(
		&self,
		custom_target:	bool,
		target_exec:	String,
		args_append:	bool,
		arguments:	Vec<String>,
		extra_files:	std::collections::HashMap<String, String>,
		envs:		std::collections::HashMap<String, String>,
		pty:		zbus::zvariant::OwnedFd,
	) -> zbus::fdo::Result<()> {
		#[cfg(debug_assertions)]
		{
			let mut log_msg = String::from("Got start request from D-Bus: ");
			log_msg.push_str(format!("Custom target: {custom_target}; ").as_str());
			log_msg.push_str(format!("target: {target_exec}; ").as_str());
			log_msg.push_str(format!("append arguments: {args_append}; ").as_str());
			log_msg.push_str(format!("arguments: {arguments:?}; ").as_str());
			log_msg.push_str(format!("extra files: {extra_files:?}; ").as_str());
			log_msg.push_str(format!("variables: {envs:?}; ").as_str());
			crate::logger::log_debug(log_msg);
		};



		let mut args: Vec<String> = vec![];

		if extra_files.len() > 0 {
			match self.replacer.add(extra_files).await {
				Ok(_)	=> {}
				Err(e)	=> {
					return Err(zbus::fdo::Error::Failed(format!("{e:#?}")))
				}
			};
		};


		let target: String = {
			if custom_target {
				target_exec.into()
			} else {
				self.conf.target.clone()
			}
		};


		if args_append {
			for val in self.conf.args.iter() {
				args.push(val.clone());
			};
		}

		args.extend(arguments);

		self.spawner.spawn(
			crate::spawn::SpawnMessage::Start {
				target: target,
				args: args,
				stream: crate::spawn::StreamConsole::WithPty {
					fd:	{
						match std::os::fd::OwnedFd::try_from(pty) {
							Ok(v)	=> {v}
							Err(e)	=> {
								return Err(
									zbus::fdo::Error::Failed(
										format!("{e:#?}"),
									),
								);
							}
						}
					},
				},
				// stream: crate::spawn::StreamConsole::Direct,
				envs: {
					if envs.len() > 0 {
						Some(envs)
					} else {
						None
					}
				},
			}
		).await;

		#[cfg(debug_assertions)]
		crate::logger::log_debug(format!("Sent spawn instructions"));

		Ok(())
	}

	#[zbus(
		name = "AuxStart2",
		out_args("master_fd")
	)]
	async fn request_start (
		&self,
		_custom_target: bool,
		_target_exec: String,
		_args_append: bool,
		_arguments: Vec<String>,
		_extra_files: std::collections::HashMap<String, String>,
		_envs: std::collections::HashMap<String, String>,
	) -> zbus::fdo::Result<zbus::zvariant::OwnedFd> {
		Err(
			zbus::fdo::Error::NotSupported("The AuxStart2 interface is deprecated".into())
		)
	}

	#[zbus(
		name = "RequestFSAccess",
	)]
	async fn request_file_system_access (
		&self,
		directory: bool
	) {
		let naming: String = match directory {
			true	=> format!("directories"),
			false	=> format!("files"),
		};
		let files = ashpd::desktop::file_chooser::SelectedFiles::open_file()
			.directory(directory)
			.title(format!("Import {naming}").as_str())
			.accept_label("Confirm")
			.modal(true)
			.multiple(true)
			.send()
			.await;
		let files = match files {
			Ok(v)	=> v,
			Err(e)	=> {
				crate::logger::log_warn(
					format!("Could not request filesystem access: {e:#?}"),
				);
				return
			},
		};

		let files = match files.response() {
			Ok(v)	=> v,
			Err(e)	=> {
				crate::logger::log_warn(
					format!("Could not request filesystem access: {e:#?}"),
				);
				return
			},
		};

		let uris = files.uris();
		let mut selected_paths: Vec<String> = vec![];
		for uri in uris.iter() {
			let pth = urlencoding::decode(uri.as_str());
			let pth = match pth {
				Ok(v)	=> v,
				Err(e)	=> {
					crate::logger::log_warn(
						format!("Could not convert {uri} to String: {e:#?}")
					);
					continue;
				}
			};
			let result = pth.strip_prefix("file://");
			match result {
				Some(v)	=> {
					selected_paths.push(v.to_string());
				}
				None	=> {
					crate::logger::log_warn(
						"Error decoding Portal response: file:// prefix not found".into(),
					);
					return;
				}
			}
		};

		crate::logger::log_debug(format!("Got response from portal: {selected_paths:?}"));

		let home = std::env::home_dir();
		let home = match home {
			Some(v)	=>	v,
			None	=>	{
				crate::logger::log_warn("Could not locate $HOME".into());
				return;
			}
		};

		let mut shared_dir = home;
		shared_dir.push("Shared");

		match std::fs::exists(shared_dir.as_path()) {
			Ok(v)	=> {
				if v == true {} else {
					match std::fs::create_dir(shared_dir.as_path()) {
						Ok(_)	=> {}
						Err(e)	=> {
							crate::logger::log_warn(
							format!(
							"Could not create shared directory: {e:#?}",
							),
						);
						return;
						}
					};
				}
			}
			Err(e)	=> {
				crate::logger::log_warn(
					format!(
						"Could not detect shared directory: {e:#?}",
					),
				);
				return;
			}
		}

		let mut map = std::collections::HashMap::<String, String>::new();

		for file in selected_paths {
			let mut dest = shared_dir.clone();
			let source = std::path::PathBuf::from(file);
			let file_name = source.file_name();
			let file_name = match file_name {
				Some(v)	=> {v}
				None	=> {
					crate::logger::log_warn(
						format!("Could not resolve file path for: {source:#?}"),
					);
					continue;
				}
			};
			dest.push(file_name);

			crate::logger::log_debug(
				format!("Linking {dest:?} to {source:?}"),
			);

			let result = std::os::unix::fs::symlink(
				&source,
				&dest,
			);
			match result {
				Ok(_)	=> {}
				Err(e)	=> {
					crate::logger::log_warn(
						format!("Could not link shared file: {e:#?}"),
					);
					continue;
				}
			};
			map.insert(
				source.into_os_string().into_string().unwrap(),
				dest.into_os_string().into_string().unwrap(),
			);
		};
		let result = self.replacer.add(map).await;
		match result {
			Ok(_)	=> {}
			Err(e)	=> {
				crate::logger::log_warn(
					format!("Could not contact replacer: {e:#?}")
				);
			}
		};
		crate::cleaner::clean_shared_dir();
	}

	#[zbus(
		property
	)]
	async fn version (&self) -> String {
		env!("CARGO_PKG_VERSION_MAJOR").to_string()
	}
}

#[derive(Debug, Error)]
pub enum BusError {
	#[error("Failed connecting to session bus: {0:#?}")]
	ConnectError(zbus::Error),

	#[error("Failed shutting down bus connection: {0:#?}")]
	ShutdownError(String),

	#[error("Failed to create a bus proxy for {0}: {1:#?}")]
	CreateProxyError(String, zbus::Error),
}

#[zbus::proxy(
		default_path = "/top/kimiblock/portable/daemon",
		interface = "top.kimiblock.Portable.Controller",
)]
trait Controller {
	fn stop(&self) -> zbus::Result<()>;
}

pub struct IPC {
	connection: zbus::Connection,
	daemon_bus_name: String,
}

impl IPC {
	pub async fn request_shutdown(self: &Self) -> Result<(), BusError> {
		let dest = self.daemon_bus_name.clone();
		let proxy = ControllerProxy::builder(&self.connection)
			.destination(dest);
		let proxy = match proxy {
			Ok(val)	=>	val,
			Err(e)	=>	return Err(BusError::CreateProxyError("Stop".into(), e))
		};
		let proxy = match proxy.build().await {
			Ok(val)	=>	val,
			Err(e)	=>	return Err(BusError::CreateProxyError("Stop".into(), e))
		};
		let reply = proxy.0.call_noreply("Stop", &());
		match reply.await {
			Ok(_)	=> Ok(()),
			Err(e)	=> Err(
				BusError::ShutdownError(format!("{e:#?}"))
			)
		}
	}

	/**
		Connect to the session bus and does not publish IPC objects.

		Does not register a well-known name.
	*/
	pub async fn connect() -> Result<zbus::Connection, BusError> {
		let builder = zbus::connection::Builder::session()
			.map_err(BusError::ConnectError)
			?;

		builder
			.allow_name_replacements(false)
			.build()
			.await
			.map_err(BusError::ConnectError)
	}

	pub async fn publish(
		conf:		std::sync::Arc<crate::envs::ConfigOpts>,
		replace_ipc:	crate::process_env::Replacer,
		spawner:	crate::spawn::Spawner,
	) -> Result<Self, BusError> {

		let bus = conf.bus_conn.clone();

		let bus_name = format!("{}.Portable.Helper", conf.sandbox_id);

		bus.request_name(bus_name)
			.await
			.map_err(BusError::ConnectError)
			?;

		let daemon_name = format!("top.kimiblock.portable.{}", conf.sandbox_id);

		let result = bus
			.object_server()
			.at(
				"/top/kimiblock/portable/init",
				Init{
					replacer: replace_ipc,
					spawner: spawner,
					conf: conf.clone(),
				},
			).await;

		match result {
			Ok(_)	=> {
				Ok(
					Self {
						connection: bus,
						daemon_bus_name: daemon_name,
					},
				)
			}

			Err(e)	=> Err(BusError::ConnectError(e))
		}
	}

	pub async fn graceful_shutdown (self: Self) -> Result<(), BusError> {
		match self.connection.close().await {
			Ok(_)	=> {Ok(())}
			Err(e)	=> {
				Err(
					BusError::ShutdownError(format!("{e:#?}"))
				)
			}
		}
	}
}



// Caller should call cancel on tokio manually
//pub fn stop_sandbox()
