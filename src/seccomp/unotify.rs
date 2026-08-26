/**
	Run this function in a dedicated thread!

	Process seccomp userspace notifications,
		take a file descriptor and cancel token to coordinate shutdown.
*/
pub fn process_seccomp_unotify (
	fd: libseccomp::ScmpFd,
	cancel_token: tokio_util::sync::CancellationToken,
) {

	let errno_raw = - {
		use nix::errno::Errno;
		let err = Errno::ENOSYS;
		err as i32
	};

	let fake_allow: Vec<String> = vec![
		"chroot".into(),
		"capset".into(),
		"setfsuid".into(),
	];

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
			if fake_allow.contains(&syscall_name) {
				libseccomp::ScmpNotifResp::new_val(
					request.id,
					0,
					libseccomp::ScmpNotifRespFlags::empty(),
				)
			} else {
				libseccomp::ScmpNotifResp::new_error(
					request.id,
					errno_raw,
					libseccomp::ScmpNotifRespFlags::empty(),
				)
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

