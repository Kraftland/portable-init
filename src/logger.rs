#[cfg(not(debug_assertions))]
pub fn log_debug(_msg: String) {}

#[cfg(debug_assertions)]
pub fn log_debug(msg: String) {
	println!(
		"\x1b[38;2;125;241;118m[Init]\x1b[0m: {}",
		msg,
	)
}

pub fn log_info(msg: String) {
	println!(
		"\x1b[38;2;119;222;250m[Init]\x1b[0m: {}",
		msg,
	)
}

pub fn log_warn(msg: String) {
	println!(
		"\x1b[38;2;255;209;59m[Init]\x1b[0m: {}",
		msg,
	)
}

pub fn log_fatal(msg: String) {
	println!(
		"\x1b[38;2;255;0;0m[Init]\x1b[0m: {}",
		msg,
	);
	nix::sys::signal::kill(
		nix::unistd::Pid::this(),
		nix::sys::signal::SIGTERM,
	);
	std::thread::sleep(std::time::Duration::from_secs(5));
	panic!("Init did not terminate on error")
}
