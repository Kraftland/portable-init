use thiserror::Error;
use std::ffi::OsString;

const INIT_APIVER: u32 = 18;

struct Init {
	replacer:	crate::process_env::Replacer,
	logtx:		tokio::sync::mpsc::Sender<crate::logger::LogMessage>,
}

#[zbus::interface(
	name = "top.kimiblock.Portable.Init",
	introspection_docs = true,
)]
impl Init {
	#[zbus(
		name = "AuxStart",
		out_args("is_stream", "base_directory")
	)]
	async fn request_start (
		&self,
		custom_target: bool,
		tray_activate: bool,
		target_exec: Vec<String>,
		arguments: Vec<String>,
		extra_files: std::collections::HashMap<String, String>,
	) -> zbus::fdo::Result<(bool, String)> {

		// TODO: replace stub
		Ok((false, "".into()))
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
				crate::logger::log(
					&self.logtx,
					crate::logger::Loglevel::Warn,
					format!("Could not request filesystem access: {e:#?}")
				).await;
				return
			},
		};

		let files = match files.response() {
			Ok(v)	=> v,
			Err(e)	=> {
				crate::logger::log(
					&self.logtx,
					crate::logger::Loglevel::Warn,
					format!("Could not request filesystem access: {e:#?}")
				).await;
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
					crate::logger::log(
						&self.logtx,
						crate::logger::Loglevel::Warn,
						format!(
						"Error decoding Portal response: file:// prefix not found",
						)
					).await;
					return;
				}
			}
		};

		crate::logger::log(
			&self.logtx,
			crate::logger::Loglevel::Debug,
			format!("Got response from portal: {selected_paths:?}"),
		).await;

		let home = std::env::home_dir();
		let home = match home {
			Some(v)	=>	v,
			None	=>	{
				crate::logger::log(
					&self.logtx,
					crate::logger::Loglevel::Warn,
					format!(
						"Could not locate $HOME",
					)
				).await;
				return;
			}
		};

		let mut shared_dir = home;
		shared_dir.push("Shared");

		let result = std::fs::create_dir(shared_dir.as_path());
		match result {
			Ok(_)	=> {},
			Err(e)	=> {
				crate::logger::log(
					&self.logtx,
					crate::logger::Loglevel::Warn,
					format!(
						"Could not create shared directory: {e:#?}",
					)
				).await;
				return;
			}
		}

		let mut map = std::collections::HashMap::<OsString, OsString>::new();

		for file in selected_paths {
			let mut dest = shared_dir.clone();
			let source = std::path::PathBuf::from(file);
			let file_name = source.file_name();
			let file_name = match file_name {
				Some(v)	=> {v}
				None	=> {
					crate::logger::log(
						&self.logtx,
						crate::logger::Loglevel::Warn,
						format!(
						"Could not resolve file path for: {source:#?}",
						)
					).await;
					continue;
				}
			};
			dest.push(file_name);

			crate::logger::log(
				&self.logtx,
				crate::logger::Loglevel::Debug,
				format!(
					"Linking {dest:?} to {source:?}",
				),
			).await;

			let result = std::os::unix::fs::symlink(
				&source,
				&dest,
			);
			match result {
				Ok(_)	=> {}
				Err(e)	=> {
					crate::logger::log(
						&self.logtx,
						crate::logger::Loglevel::Warn,
						format!(
						"Could not link shared file: {e:#?}",
						)
					).await;
					continue;
				}
			};
			map.insert(source.into_os_string(), dest.into_os_string());
		};
		let result = self.replacer.add(map).await;
		match result {
			Ok(_)	=> {}
			Err(e)	=> {
				crate::logger::log(
					&self.logtx,
					crate::logger::Loglevel::Warn,
					format!(
					"Could not contact replacer: {e:#?}",
					)
				).await;
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
		logtx: tokio::sync::mpsc::Sender<crate::logger::LogMessage>,
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
					logtx: logtx,
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
