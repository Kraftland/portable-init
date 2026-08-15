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
}

#[derive(Debug, Clone)]
pub struct ConfigOpts {
	pub lockdown:		bool,
	pub has_flatpak_info:	bool,
	pub debugging:		bool,
	pub sandbox_id:		String,

	// Origin -> dest
	pub file_map:		std::collections::HashMap<String, String>,

	pub inhibit:		bool,

	pub target:		String,
	pub args:		Vec<String>,
	pub bus_conn:		zbus::Connection,
	pub uclamp_min:		u8,
	pub uclamp_max:		u8,
}

/**
	Get configurations via D-Bus IPC
*/
pub async fn get() -> Result<std::sync::Arc<ConfigOpts>, EnvsError> {

	let appid = app_id::get()?;

	let daemon_name = format!("top.kimiblock.portable.{}", &appid);

	let bus_connection = crate::ipc::IPC::connect()
		.await
		.map_err(EnvsError::ConnectBusError)
		?;

	let init_config = bus::get(&bus_connection, &daemon_name)
		.await
		.map_err(EnvsError::BusError)
		?;

	Ok(
		std::sync::Arc::new(
			ConfigOpts {
				lockdown:		init_config.lockdown,
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
			}
		)
	)
}
