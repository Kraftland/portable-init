use thiserror::Error;

mod bus;
mod app_id;

#[derive(Error, Debug)]
pub enum EnvsError {
	#[error("D-Bus error resolving configuration: {0:#?}")]
	BusError(zbus::Error),

	#[error("Error connecting to D-Bus")]
	ConnectBusError(crate::ipc::BusError),

	#[error("Argument mismatch for command line")]
	ArgError,

	#[error("FD conversion error: {0:#?}")]
	FDConvertError(std::convert::Infallible),

	#[error("Spawn error: {0:#?}")]
	SpawnError(tokio::task::JoinError),

	#[error("PIDFD error: {0:#?}")]
	PidFdError(i64),

	#[error("fstat on PIDFD error: {0:#?}")]
	PidFdFStatError(nix::Error),
}

#[derive(Debug)]
pub struct ConfigOpts {
	pub landlock:		bool,
	pub seccomp_whitelist:	bool,
	pub has_flatpak_info:	bool,
	pub debugging:		bool,
	pub sandbox_id:		String,

	// Origin -> dest
	pub file_map:		std::collections::HashMap<String, String>,

	pub inhibit:		bool,

	pub target:		String,
	pub args:		Vec<String>,
	pub bus_conn:		zbus::Connection,
	pub uclamp_min:		u32,
	pub uclamp_max:		u32,

	pub pty_fd:		Option<std::os::fd::OwnedFd>,

	/**
		The PIDFD inode number for Init
	*/
	pub pidfd_ino:		Option<u64>,
}

/**
	Get configurations via D-Bus IPC
*/
pub async fn get() -> Result<std::sync::Arc<ConfigOpts>, EnvsError> {

	let appid = app_id::get()?;

	let daemon_name = std::sync::Arc::new(format!("top.kimiblock.portable.{}", &appid));

	let bus_connection = crate::ipc::IPC::connect()
		.await
		.map_err(EnvsError::ConnectBusError)
		?;

	let pid = tokio::spawn(
		bus::get_pidfd_inode()
	);

	let init_config = bus::get(&bus_connection, daemon_name)
		.await
		?;

	let pid = match pid.await.map_err(EnvsError::SpawnError)? {
		Ok(v)	=> Some(v),
		Err(e)	=> {
			crate::logger::log_warn(
				format!("Could not get host PID: {e:#?}")
			);
			None
		}
	};

	Ok(
		std::sync::Arc::new(
			ConfigOpts {
				landlock:		init_config.landlock,
				seccomp_whitelist:	init_config.seccomp_whitelist,
				has_flatpak_info:	init_config.flatpak_info,
				debugging:		init_config.allow_debug,
				sandbox_id:		appid,
				file_map:		init_config.extra_files,
				inhibit:		init_config.inhibit_suspend,
				target:			init_config.target_exec,
				args:			init_config.target_args,
				bus_conn:		bus_connection,
				uclamp_min:		init_config.uclamp_min,
				uclamp_max:		init_config.uclamp_max,
				pty_fd:			init_config.pty_fd,
				pidfd_ino:		pid,
			}
		)
	)
}
