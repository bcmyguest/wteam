//! Translate a profile for pi. pi has no permission system, so every rule is
//! unsupported; only the bypass profile is exact (pi runs tools unrestricted).

use anyhow::Result;

use super::{Profile, RuleSupport, Support, Translation, rules_of};

const FALLBACK: &str = "pi runs tools unrestricted; use a sandbox";

pub fn translate(profile: &Profile, role: &str) -> Result<Translation> {
    let _ = role;
    let support = rules_of(profile)?
        .into_iter()
        .map(|(decision, rule, _)| RuleSupport {
            decision,
            rule,
            support: if profile.bypass {
                Support::Exact
            } else {
                Support::Unsupported(FALLBACK.into())
            },
        })
        .collect();
    Ok(Translation {
        support,
        ..Translation::default()
    })
}
