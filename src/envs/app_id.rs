
/**
	Obtain the application ID

	app_id is now passed as argument 1
*/
pub fn get() -> Result<String, super::EnvsError> {
	let args = std::env::args();

	if args.len() != 2 {
		return Err(super::EnvsError::ArgError);
	};

	let mut iter = args.skip(1);

	match iter.next() {
		Some(v)	=> {
			Ok(v)
		}
		None	=> {
			Err(super::EnvsError::ArgError)
		}
	}
}
