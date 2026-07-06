use thiserror::Error;

const INIT_APIVER: u32 = 18;

struct Init {
	replacer:	crate::process_env::Replacer,
	spawner:	crate::spawn::Spawner,
	conf:		crate::envs::ConfigOpts,
}

#[derive(Debug, zbus::DBusError)]
enum AuxStartError {
	RecvError(String),
	ReplaceError(String),
}

#[zbus::interface(
	name = "top.kimiblock.Portable.Init",
	introspection_docs = true,
)]
impl Init {
	#[zbus(
		name = "AuxStart2",
		out_args("master_fd")
	)]
	async fn request_start (
		&self,
		custom_target: bool,
		target_exec: String,
		args_append: bool,
		arguments: Vec<String>,
		extra_files: std::collections::HashMap<String, String>,
		envs: std::collections::HashMap<String, String>,
	) -> Result<zbus::zvariant::OwnedFd, AuxStartError> {
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
					return Err(AuxStartError::ReplaceError(format!("{e:#?}")))
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

		for val in arguments {
			args.push(val.into());
		};


		let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();

		let envs_req = {

			if envs.len() > 0 {
				let mut map = std::collections::HashMap::<String, String>::new();
				for (k, v) in envs {
					map.insert(k.into(), v.into());
				}
				Some(map)
			} else {
				None
			}
		};

		self.spawner.spawn(
			crate::spawn::SpawnMessage::Start {
				target: target,
				args: args,
				stream: true,
				reply: Some(reply_tx),
				envs: envs_req,
			}
		).await;

		let reply = reply_rx.await;
		match reply {
			Ok(v)	=> {
				Ok(zbus::zvariant::OwnedFd::from(v.master_fd))
			}
			Err(e)	=> {
				Err(AuxStartError::RecvError(format!("{e:#?}")))
			}
		}
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
			let pth = uri.as_str();
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
		}
	}

	#[zbus(
		property
	)]
	async fn version (&self) -> u32 {
		INIT_APIVER
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

	pub async fn connect(
		conf: &crate::envs::ConfigOpts,
		replace_ipc: crate::process_env::Replacer,
		spawner: crate::spawn::Spawner,
	) -> Result<Self, BusError> {
		let conn = zbus::connection::Builder::session();
		let conn = match conn {
			Ok(val)	=> val,
			Err(e)	=> return Err(BusError::ConnectError(e))
		};

		let bus_name = format!("{}.Portable.Helper", conf.sandbox_id);

		let conn = match conn.name(bus_name) {
			Ok(val)	=> val,
			Err(e)	=> return Err(BusError::ConnectError(e))
		};

		let conn = conn.allow_name_replacements(false);

		let daemon_name = format!("top.kimiblock.portable.{}", conf.sandbox_id);

		let conn = match conn.build().await {
			Ok(val)	=> val,
			Err(e)	=> return Err(BusError::ConnectError(e))
		};

		let result = conn.object_server()
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
						connection: conn,
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
