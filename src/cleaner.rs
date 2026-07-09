

// spawn this in an OS-thread?
pub fn clean_shared_dir() {
	let builder = std::thread::Builder::new()
		.name("init-cleaner".into());
	builder.spawn(|| {
		let home = {
			match std::env::home_dir() {
				Some(v)	=> v,
				None	=> {
					crate::logger::log_warn(
					format!("Could not clean shared directory: unable to locate home")
					);
					return;
				}
			}
		};
		let mut shared_dir_path = home;
		shared_dir_path.push("Shared");
		let entries = match std::fs::read_dir(shared_dir_path) {
			Ok(v)	=> v,
			Err(e)	=> {
				crate::logger::log_warn(
					format!("Could not clean shared directory: {e:#?}"),
				);
				return;
			}
		};
	});

}
