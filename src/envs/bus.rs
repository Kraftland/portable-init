#[zbus::proxy(
	interface	= "top.kimiblock.portable.InitInfo",
	default_path	= "/top/kimiblock/portable/daemon",
)]
trait Info {
	#[zbus(
		name	= "GetInfo",
		property,
	)]
	fn get(&self) -> zbus::Result<(
		std::collections::HashMap<String, String>,
		bool,
		bool,
		bool,
		bool,
		String,
		Vec<String>,
		u32,
		u32,
	)>;
}

/**
	The public struct InitInfo describes information passed down to Init via bus IPC

	It is estimated that passing them directly instead of using memfd is faster at smaller
	quantity.

	With the new way of passing down information, Init will be supplied of only an app_id, and
	will therefore contact the controller. Thus, we can manipulate the started atomic boolean
	inside AuxStart struct to clearly indicate whether the Init system has started.
*/
pub struct InitInfo {
	pub extra_files:	std::collections::HashMap<String, String>,
	pub inhibit_suspend:	bool,
	pub flatpak_info:	bool,

	/**
		Lockdown is an alias of seccomp whitelist + landlock
	*/
	pub lockdown:		bool,

	/**
		Whether or not to allow a set of debugging syscalls
	*/
	pub allow_debug:	bool,

	/**
		Designates the target executable to start upon.

		Care should be taken when constructing this field, because debug shell and
			D-Bus activation can have different target executable.
	*/
	pub target_exec:	String,

	/**
		An array of strings describing the arguments to pass
	*/
	pub target_args:	Vec<String>,

	/**
		uclamp_min describes the minimum guaranteed performance operating point.

		It is clamped between 0 and 100, as per cgroup v2 specifications.
	*/
	pub uclamp_min:		u32,
	/**
		uclamp_max describes the maximum performance operating point.
		It operates as a ceiling limit.

		It is clamped between 0 and 100, as per cgroup v2 specifications.
	*/
	pub uclamp_max:		u32,
}

/**
	The public struct InitInfo describes information passed down to Init via bus IPC

	It is estimated that passing them directly instead of using memfd is faster at smaller
	quantity.

	With the new way of passing down information, Init will be supplied of only an app_id, and
	will therefore contact the controller. Thus, we can manipulate the started atomic boolean
	inside AuxStart struct to clearly indicate whether the Init system has started.
*/
pub async fn get(bus: &zbus::Connection, daemon_name: &str) -> Result<InitInfo, zbus::Error> {
	let proxy = InfoProxy::new(bus, daemon_name)
		.await
		?;

	let info = proxy.get().await?;

	let ret = InitInfo {
		extra_files:		info.0,
		inhibit_suspend:	info.1,
		flatpak_info:		info.2,
		lockdown:		info.3,
		allow_debug:		info.4,
		target_exec:		info.5,
		target_args:		info.6,
		uclamp_min:		info.7,
		uclamp_max:		info.8,
	};

	Ok(ret)
}
