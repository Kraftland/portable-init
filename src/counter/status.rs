pub enum SandboxStatus {
	/**
		Sandbox is running and tracking a certain amount of processes
	*/
	Ready { tracked_pid: usize },

	/**
		Sandbox is stopping
	*/
	Stopping,
}

pub mod systemd;

/**
	The public trait Init is used to initialise status subsystems.

	For example, systemd may need a "Ready status"
*/
pub trait Init {
	async fn initialise(&self) -> Result<(), Self::StatusError>;

	type StatusError;
}

/**
	The public trait UpdateStatus is used to update the SandboxStatus for a certain subsystem.
*/
pub trait UpdateStatus {
	async fn update(&self, status: &SandboxStatus) -> Result<(), Self::StatusError>;

	type StatusError;
}
