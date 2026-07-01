use thiserror::Error;
use landlock::{
	RulesetAttr,
	AccessFs,
	Compatible,
	CompatLevel,
	RulesetCreatedAttr,
};

#[derive(Debug, Error)]
pub enum LandlockError {
	#[error("Init filter error {0:#?}")]
	InitFilterError(landlock::RulesetError),
	#[error("Add rules error {0:#?}")]
	AddRulesError(landlock::RulesetError),
	#[error("Apply rules error {0:#?}")]
	ApplyRulesError(landlock::RulesetError),
	#[error("Unexpected status reply: {0:#?}")]
	ApplyStatusError(String),
	#[error("Unable to determine home")]
	UserHomeUnknownError,
}

struct DefinedRules {
	RO:		Vec<String>,
	RW:		Vec<String>,
	Full:		Vec<String>,
	ReadDir:	Vec<String>,
}

impl DefinedRules {
	fn get(has_flatpak_info: &bool) -> Result<Self, LandlockError> {
		let home = std::env::home_dir();
		let home = match home {
			Some(val)	=> val,
			None		=> {
				return Err(LandlockError::UserHomeUnknownError);
			}
		};

		let mut ret = DefinedRules {
			RO: vec![
				"/bin".into(),
				"/sys/fs/cgroup".into(),
				"/etc".into(),
				"/lib".into(),
				"lib64".into(),
				"/opt".into(),
				"/sbin".into(),
				"/usr".into(),
			],
			RW: vec![
				home.to_string_lossy().to_string(),
				"/run".into(),
				"/tmp".into(),
			],
			Full: vec![
				"/dev".into(),
				"/proc".into(),
				"/sys".into(),
			],
			ReadDir: vec![
				"/".into(),
			],
		};
		if *has_flatpak_info == true {
			ret.RO.push("/.flatpak-info".into());
		}
		Ok(ret)
	}
}

pub fn load_landlock (conf: &crate::envs::ConfigOpts) -> Result<(), LandlockError> {
	let defined_rules = DefinedRules::get(&conf.has_flatpak_info)?;


	enum LandlockFsAccess {
		Full,
		Directory, // Does not include MakeChar, MakeBlock, IoctlDev
		DirectoryRO,
		Empty,
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
				},
				Self::Empty => {
					landlock::BitFlags::<AccessFs>::empty()
				}
			}
		}
	}

	let rule_set = landlock::Ruleset::default()
		.handle_access(
			LandlockFsAccess::Empty.rule(),
		);
	let rule = match rule_set {
		Ok(val)	=> val,
		Err(e)	=> {
			return Err(LandlockError::InitFilterError(e));
		},
	};

	let rule = rule.set_compatibility(CompatLevel::HardRequirement).create();
	let rule_set = match rule {
		Ok(val)	=> val,
		Err(e)	=> {return Err(LandlockError::InitFilterError(e));},
	};


	let result = rule_set.add_rules(
		landlock::path_beneath_rules(
			defined_rules.RO,
			LandlockFsAccess::DirectoryRO.rule(),
		),
	);
	let rule_set = match result {
		Ok(val)	=> val,
		Err(e)	=> {
			return Err(LandlockError::AddRulesError(e));
		}
	};

	let result = rule_set.add_rules(
		landlock::path_beneath_rules(
			defined_rules.RW,
			LandlockFsAccess::Directory.rule(),
		),
	);
	let rule_set = match result {
		Ok(val)	=> val,
		Err(e)	=> {
			return Err(LandlockError::AddRulesError(e));
		}
	};

	let result = rule_set.add_rules(
		landlock::path_beneath_rules(
			defined_rules.Full,
			LandlockFsAccess::Full.rule(),
		),
	);
	let rule_set = match result {
		Ok(val)	=> val,
		Err(e)	=> {
			return Err(LandlockError::AddRulesError(e));
		}
	};

	let result = rule_set.add_rules(
		landlock::path_beneath_rules(
			defined_rules.ReadDir,
			landlock::make_bitflags!(AccessFs::ReadDir),
		),
	);
	let rule_set = match result {
		Ok(val)	=> val,
		Err(e)	=> {
			return Err(LandlockError::AddRulesError(e));
		}
	};

	let status = rule_set.restrict_self();
	let status = match status {
		Ok(val)	=> val,
		Err(e)	=> {
			return Err(LandlockError::ApplyRulesError(e));
		}
	};

	match status.ruleset {
		landlock::RulesetStatus::NotEnforced	=> {
			return Err(
				LandlockError::ApplyStatusError(
					"Not enforced. Please upgrade kernel".into(),
				),
			);
		}
		landlock::RulesetStatus::FullyEnforced => {Ok(())}
		landlock::RulesetStatus::PartiallyEnforced => {
			return Err(
				LandlockError::ApplyStatusError(
					"Not fully enforced".into()
				)
			);
		}
	}
}
