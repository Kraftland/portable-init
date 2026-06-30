use thiserror::Error;
use landlock::{
	RulesetAttr,
	AccessFs,
};

#[derive(Debug, Error)]
pub enum LandlockError {}

pub fn load_landlock (conf: &crate::envs::ConfigOpts) -> Result<(), LandlockError> {


	enum LandlockFsAccess {
		Full,
		Directory, // Does not include MakeChar, MakeBlock, IoctlDev
		DirectoryRO,
	}
	impl LandlockFsAccess {
		fn rule(self: &Self) -> landlock::BitFlags<AccessFs> {
			match self {
				Self::Full => landlock::BitFlags::<AccessFs>::all(),
				Self::Directory => {
					let mut rule = landlock::BitFlags::<AccessFs>::all();
					rule.remove(AccessFs::MakeChar);
					rule.remove(AccessFs::MakeBlock);
					rule.remove(AccessFs::IoctlDev);
					rule
				},
				Self::DirectoryRO => {
					landlock::make_bitflags!(
						AccessFs::{
							Execute		|
							ReadDir		|
							ReadFile
						}
					)
				}
			}
		}
	}
	//let rule_set = landlock::Ruleset::default().handle_access();

	Ok(())
}
