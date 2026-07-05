use thiserror::Error;
use landlock::{
	RulesetAttr,
	AccessFs,
	Compatible,
	CompatLevel,
	Access,
	RulesetCreatedAttr,
};

const ABI: landlock::ABI = landlock::ABI::V6;

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
	#[error("Unable to clone landlock rule-set: {0:#?}")]
	CloneError(std::io::Error),
}

struct DefinedRules {
	ro:		Vec<String>,
	rw:		Vec<String>,
	full:		Vec<String>,
	read_dir:	Vec<String>,
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
			ro: vec![
				"/bin".into(),
				"/sys/fs/cgroup".into(),
				"/etc".into(),
				"/lib".into(),
				"/lib64".into(),
				"/opt".into(),
				"/sbin".into(),
				"/usr".into(),
			],
			rw: vec![
				home.to_string_lossy().to_string(),
				"/run".into(),
				"/tmp".into(),
			],
			full: vec![
				"/dev".into(),
				"/proc".into(),
				"/sys".into(),
			],
			read_dir: vec![
				"/".into(),
			],
		};
		if *has_flatpak_info == true {
			ret.ro.push("/.flatpak-info".into());
		}
		Ok(ret)
	}
}

pub async fn compile_landlock_rules (conf: &crate::envs::ConfigOpts) -> Result<landlock::RulesetCreated, LandlockError> {
	let defined_rules = DefinedRules::get(&conf.has_flatpak_info);
	let defined_rules = match defined_rules {
		Ok(v)	=>	v,
		Err(e)	=> 	{
			return Err(
				e
			)
		}
	};
	enum LandlockFsAccess {
		Full,
		Directory, // Does not include MakeChar, MakeBlock, IoctlDev
		DirectoryRO,
	}
	impl LandlockFsAccess {
		fn rule(self: &Self) -> landlock::BitFlags<AccessFs> {
			match self {
				Self::Full => AccessFs::from_all(ABI),
				Self::Directory => {
					let mut rule = AccessFs::from_all(ABI);
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

	let rule_set = landlock::Ruleset::default()
		.handle_access(
			LandlockFsAccess::Full.rule(),
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
			defined_rules.ro,
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
			defined_rules.rw,
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
			defined_rules.full,
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
			defined_rules.read_dir,
			landlock::make_bitflags!(AccessFs::ReadDir),
		),
	);
	match result {
		Ok(val)	=> Ok(val),
		Err(e)	=> {
			return Err(LandlockError::AddRulesError(e));
		}
	}
}

pub fn load_landlock (rule: landlock::RulesetCreated) -> Result<(), LandlockError> {
	let rule_set = rule;

	let _scope = landlock::Scope::from(landlock::Scope::Signal);

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

