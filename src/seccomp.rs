use thiserror::Error;

#[derive(Error, Debug)]
pub enum SeccompError {
	#[error("Could not create seccomp filter: {0:?}")]
	CreateFilterError(libseccomp::error::SeccompError),

	#[error("Could not add filter rule: {0:?}")]
	AddRuleError(libseccomp::error::SeccompError)
}

// Loads a Secure Computing filter
pub fn load_seccomp_filter (config_env: &crate::envs::ConfigOpts) -> Result<(), SeccompError> {
	let filter = libseccomp::ScmpFilterContext::new(
		libseccomp::ScmpAction::Allow,
	);

	let mut filter = match filter {
		Ok(res)	=>	res,
		Err(e)	=>	{
			return Err(SeccompError::CreateFilterError(e));
		},
	};

	let filter_result = filter.add_arch(
		libseccomp::ScmpArch::Native
	);
	match filter_result {
		Ok(_)	=>	{},
		Err(e)	=>	{
			return Err(SeccompError::AddRuleError(e));
		},
	}

	let filter_result = match config_env.lockdown {
		true	=>	{
			filter.set_act_badarch(
				libseccomp::ScmpAction::Errno(
					std::io::Error::new(
						std::io::ErrorKind::Unsupported,
						"Architecture unsupported",
					)
						.raw_os_error()
						.unwrap()
			))
		}
		false	=>	{
			filter.set_act_badarch(libseccomp::ScmpAction::Allow)
		}
	};
	match filter_result {
		Ok(_)	=>	{},
		Err(e)	=>	{
			return Err(SeccompError::AddRuleError(e));
		},
	}



	Ok(())
}

