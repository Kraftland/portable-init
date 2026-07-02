use thiserror::Error;

#[derive(Debug, Error)]
pub enum BusError {
	#[error("Failed connecting to session bus: {0:#?}")]
	ConnectError(zbus::Error)
}



pub struct IPC {
	connection: zbus::Connection
}

impl IPC {
	pub async fn connect(conf: &crate::envs::ConfigOpts) -> Result<zbus::Connection, BusError> {
		let conn = zbus::connection::Builder::session();
		let conn = match conn {
			Ok(val)	=> val,
			Err(e)	=> return Err(BusError::ConnectError(e))
		};

		let bus_name = String::from("top.kimiblock.portable.").push_str(&conf.sandbox_id);

		let conn = match conn.name(bus_name) {
			Ok(val)	=> val,
			Err(e)	=> return Err(BusError::ConnectError(e))
		};

		let conn = conn.allow_name_replacements(false);

		match conn.build().await {
			Ok(val)	=> Ok(val),
			Err(e)	=> Err(BusError::ConnectError(e))
		}
	}
}



// Caller should call cancel on tokio manually
//pub fn stop_sandbox()
