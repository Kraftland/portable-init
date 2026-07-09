

// spawn this in an OS-thread?
pub fn clean_shared_dir() {
	let builder = std::thread::Builder::new()
		.name("init-cleaner".into());
	let _ = builder.spawn(|| {
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
		for entry in entries {
			let entry = match entry {
				Ok(v)	=> {v}
				Err(e)	=> {
					crate::logger::log_warn(
						format!("Could not read shared file: {e:#?}"),
					);
					continue;
				}
			};
			let path = entry.path();
			let meta = std::fs::symlink_metadata(&path);
			match meta {
				Ok(v)	=> {
					if ! v.is_symlink() {
						continue;
					};
				}
				Err(e)	=> {
					crate::logger::log_warn(
						format!("Could not read shared file: {e:#?}"),
					);
					continue;
				}
			};
			match std::fs::metadata(&path) {
				Ok(_)	=> {
					crate::logger::log_debug(
						format!("{path:?} appears to be up"),
					);
					continue;
				}
				Err(e)	=> {
					crate::logger::log_debug(
						format!("{path:?} appears to be broken symlink: {e:#?}"),
					);
				}
			};
			match std::fs::remove_file(&path) {
				Ok(_)	=> {
					crate::logger::log_debug(
						format!("Removed expired file {path:?}"),
					);
				}
				Err(e)	=> {
					crate::logger::log_warn(
						format!("Could not remove broken symlink {path:?}: {e:#?}"),
					);
				}
			};
		};
	});

}
