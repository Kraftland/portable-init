/**
	Run this function in a dedicated thread!

	Process seccomp userspace notifications,
		take a file descriptor and cancel token to coordinate shutdown.
*/
pub fn process_seccomp_unotify (
	fd: libseccomp::ScmpFd,
	cancel_token: tokio_util::sync::CancellationToken,
) {
	// (syscall name (str), return value, error value)
	let mut override_map = std::collections::HashMap::new();
	{
		override_map.insert("capset", (0, 0));
		override_map.insert("unshare", (0, 0));
		override_map.insert("clone", (0, 0));
		override_map.insert("clone3", (0, 0));
		override_map.insert("chroot", (0, 0));
		override_map.insert("setfsuid", (0, 0));
	};

	let errno_raw = - {
		use nix::errno::Errno;
		let err = Errno::ENOSYS;
		err as i32
	};

	loop {
		let request = libseccomp::ScmpNotifReq::receive(fd);
		if cancel_token.is_cancelled() {
			return
		}
		let request = match request {
			Ok(val)	=> val,
			Err(e)	=> {
				eprintln!("Could not receive seccomp notification: {e:#?}");
				return
			}
		};

		let syscall_name = request.data.syscall.get_name();
		let syscall_name = match syscall_name {
			Ok(val)	=> val,
			Err(e)	=> {
				eprintln!("Could not resolve syscall: {:#?}", e);
				let response = libseccomp::ScmpNotifResp::new_continue(
					request.id,
					libseccomp::ScmpNotifRespFlags::empty(),
				);
				match response.respond(fd) {
					Ok(_)	=> {}
					Err(e)	=> {
						eprintln!("Could not respond to syscall: {e:#?}");
					}
				};
				continue;
			}
		};

		crate::logger::log_warn(
			format!(
				"PID {} performed illegal syscall: {} on architecture {:?}",
				&request.pid,
				syscall_name,
				&request.data.arch,
			)
		);

		let response = {
			match override_map.get(&syscall_name.as_str()) {
				Some(v)	=> {
					#[cfg(debug_assertions)]
					crate::logger::log_debug(
						format!("Overriding return value for {syscall_name}: ")
					);

					libseccomp::ScmpNotifResp::new(
						request.id,
						v.0,
						v.1,
						libseccomp::ScmpNotifRespFlags::empty().bits(),
					)
				}
				None	=> {
					libseccomp::ScmpNotifResp::new_error(
						request.id,
						errno_raw,
						libseccomp::ScmpNotifRespFlags::empty(),
					)
				}
			}
		};

		match response.respond(fd) {
			Ok(_)	=> {},
			Err(e)	=> {
				eprintln!("Could not filter system call: {e:#?}")
			},
		}
	}
}

