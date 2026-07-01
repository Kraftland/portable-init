use thiserror::Error;
use std::io::Write;

#[derive(Debug, Error)]
pub enum ApplyUclampError {
	#[error("Not unicode: {0:#?}")]
	NotUnicodeErr(std::env::VarError),

	#[error("Unable to parse variable: {0:#?}")]
	CouldNotParseErr(std::num::ParseIntError),

	#[error("Invalid uclamp.max value: {0}")]
	InvalidUclampMaxErr(u32),

	#[error("Failed to open uclamp.max: {0:#?}")]
	OpenFileErr(std::io::Error),

	#[error("Failed to write uclamp.max: {0:#?}")]
	WriteFileErr(std::io::Error),
}

pub fn apply_uclamp () -> Result<Option<u32>, ApplyUclampError> {
	let raw_env = std::env::var("_portableUclampMax");
	let uclamp_max_literal = match raw_env {
		Ok(val)	=>	{val}
		Err(e)	=>	{
			if e == std::env::VarError::NotPresent {
				return Ok(None)
			} else {
				return Err(
					ApplyUclampError::NotUnicodeErr(e),
				)
			}
		}
	};
	let result = uclamp_max_literal.parse::<u32>();
	let uclamp_max = match result {
		Ok(val)	=> val,
		Err(e)	=> return Err(ApplyUclampError::CouldNotParseErr(e))
	};
	if uclamp_max > 1024 {
		return Err(ApplyUclampError::InvalidUclampMaxErr(uclamp_max));
	};

	let mut max_file = std::fs::OpenOptions::new();
	max_file.write(true);
	max_file.append(false);
	let max_file = max_file.open("/sys/fs/cgroup/cpu.uclamp.max");
	let mut max_file = match max_file {
		Ok(val)	=> val,
		Err(e)	=> {
			return Err(ApplyUclampError::OpenFileErr(e))
		}
	};

	let result = max_file.write_fmt(format_args!("{uclamp_max_literal}"));
	match result {
		Ok(_)	=> return Ok(Some(uclamp_max)),
		Err(e)	=> return Err(ApplyUclampError::WriteFileErr(e))
	}


}
