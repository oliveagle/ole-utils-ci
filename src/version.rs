// Copyright 2026 devine_utils contributors.
// Licensed under the Apache License, Version 2.0.
// SPDX-License-Identifier: Apache-2.0

//! The version model of the tool.
//!
//! Two version shapes are legal, both valid SemVer 2.0.0:
//!
//! * stable — `x.y.z`, for example `1.1.2`
//! * release candidate — `x.y.z-rc.N`, for example `1.1.3-rc.1`
//!
//! The release-candidate lane is the pre-release convention the tool follows:
//! every upgrade starts as a candidate and is later promoted to a stable
//! version.

use std::fmt;

/// A parsed version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    /// The major component.
    pub major: u64,
    /// The minor component.
    pub minor: u64,
    /// The patch component.
    pub patch: u64,
    /// The release-candidate number; `0` for a stable version.
    pub rc: u64,
    /// Whether this is a release candidate.
    pub is_rc: bool,
}

/// The bump kinds `bump-version` understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bump {
    /// `x.y.z` to `x.y.(z+1)`.
    Patch,
    /// `x.y.z` to `x.(y+1).0`.
    Minor,
    /// `x.y.z` to `(x+1).0.0`.
    Major,
    /// A stable version to its next release candidate.
    Rc,
    /// A release candidate to its stable version.
    Promote,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_rc {
            write!(
                f,
                "{}.{}.{}-rc.{}",
                self.major, self.minor, self.patch, self.rc
            )
        } else {
            write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
        }
    }
}

impl Version {
    /// Parses a version string.
    pub fn parse(raw: &str) -> Result<Version, String> {
        let raw = raw.trim();
        let (base, rc) = match raw.split_once("-rc.") {
            Some((base, rc)) => {
                let number: u64 = rc
                    .parse()
                    .map_err(|_| format!("invalid version {raw:?} (expect x.y.z or x.y.z-rc.N)"))?;
                (base, Some(number))
            }
            None => (raw, None),
        };
        let mut parts = base.split('.');
        let major = parse_component(parts.next(), raw)?;
        let minor = parse_component(parts.next(), raw)?;
        let patch = parse_component(parts.next(), raw)?;
        if parts.next().is_some() {
            return Err(format!(
                "invalid version {raw:?} (expect x.y.z or x.y.z-rc.N)"
            ));
        }
        Ok(Version {
            major,
            minor,
            patch,
            rc: rc.unwrap_or(0),
            is_rc: rc.is_some(),
        })
    }

    /// The stable base of the version, without the release-candidate suffix.
    pub fn stable(&self) -> Version {
        Version {
            is_rc: false,
            rc: 0,
            ..*self
        }
    }

    /// The next version for `bump`.
    pub fn next(&self, bump: Bump) -> Result<Version, String> {
        match bump {
            Bump::Major => {
                self.reject_rc("bump")?;
                Ok(Version {
                    major: self.major + 1,
                    minor: 0,
                    patch: 0,
                    rc: 0,
                    is_rc: false,
                })
            }
            Bump::Minor => {
                self.reject_rc("bump")?;
                Ok(Version {
                    major: self.major,
                    minor: self.minor + 1,
                    patch: 0,
                    rc: 0,
                    is_rc: false,
                })
            }
            Bump::Patch => {
                self.reject_rc("bump")?;
                Ok(Version {
                    major: self.major,
                    minor: self.minor,
                    patch: self.patch + 1,
                    rc: 0,
                    is_rc: false,
                })
            }
            Bump::Rc => {
                if self.is_rc {
                    Ok(Version {
                        rc: self.rc + 1,
                        ..*self
                    })
                } else {
                    Ok(Version {
                        major: self.major,
                        minor: self.minor,
                        patch: self.patch + 1,
                        rc: 1,
                        is_rc: true,
                    })
                }
            }
            Bump::Promote => {
                if !self.is_rc {
                    return Err(format!(
                        "current version {self} is already stable — nothing to promote"
                    ));
                }
                Ok(self.stable())
            }
        }
    }

    /// Rejects a release candidate for the stable-only bumps.
    fn reject_rc(&self, action: &str) -> Result<(), String> {
        if self.is_rc {
            return Err(format!(
                "current version {self} is an rc candidate — run promote first to publish the stable version, then {action} again"
            ));
        }
        Ok(())
    }
}

/// Parses one numeric component of a version.
fn parse_component(part: Option<&str>, raw: &str) -> Result<u64, String> {
    let part =
        part.ok_or_else(|| format!("invalid version {raw:?} (expect x.y.z or x.y.z-rc.N)"))?;
    part.parse()
        .map_err(|_| format!("invalid version {raw:?} (expect x.y.z or x.y.z-rc.N)"))
}

/// Parses a bump kind from its command-line spelling.
pub fn parse_bump(raw: &str) -> Result<Bump, String> {
    match raw {
        "patch" | "" => Ok(Bump::Patch),
        "minor" => Ok(Bump::Minor),
        "major" => Ok(Bump::Major),
        "rc" => Ok(Bump::Rc),
        "promote" => Ok(Bump::Promote),
        other => Err(format!(
            "unknown bump type {other:?} — use patch, minor, major, rc, or promote"
        )),
    }
}

/// The canonical spelling of a bump kind.
pub fn bump_name(bump: Bump) -> &'static str {
    match bump {
        Bump::Patch => "patch",
        Bump::Minor => "minor",
        Bump::Major => "major",
        Bump::Rc => "rc",
        Bump::Promote => "promote",
    }
}

/// The expected tag name of a version.
pub fn tag_for(version: &Version) -> String {
    format!("v{version}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_stable_and_rc_versions() {
        let stable = Version::parse("1.2.3").unwrap();
        assert_eq!(stable.major, 1);
        assert_eq!(stable.minor, 2);
        assert_eq!(stable.patch, 3);
        assert!(!stable.is_rc);
        let rc = Version::parse("1.2.3-rc.4").unwrap();
        assert!(rc.is_rc);
        assert_eq!(rc.rc, 4);
        assert_eq!(rc.to_string(), "1.2.3-rc.4");
    }

    #[test]
    fn rejects_malformed_versions() {
        assert!(Version::parse("1.2").is_err());
        assert!(Version::parse("1.2.3.4").is_err());
        assert!(Version::parse("x.y.z").is_err());
        assert!(Version::parse("1.2.3-rc.x").is_err());
    }

    #[test]
    fn computes_the_next_version() {
        let stable = Version::parse("1.1.2").unwrap();
        assert_eq!(stable.next(Bump::Patch).unwrap().to_string(), "1.1.3");
        assert_eq!(stable.next(Bump::Minor).unwrap().to_string(), "1.2.0");
        assert_eq!(stable.next(Bump::Major).unwrap().to_string(), "2.0.0");
        assert_eq!(stable.next(Bump::Rc).unwrap().to_string(), "1.1.3-rc.1");
        let rc = Version::parse("1.1.3-rc.1").unwrap();
        assert_eq!(rc.next(Bump::Rc).unwrap().to_string(), "1.1.3-rc.2");
        assert_eq!(rc.next(Bump::Promote).unwrap().to_string(), "1.1.3");
    }

    #[test]
    fn stable_bumps_reject_an_rc_candidate() {
        let rc = Version::parse("1.1.3-rc.1").unwrap();
        assert!(rc.next(Bump::Patch).is_err());
        assert!(rc.next(Bump::Minor).is_err());
        assert!(rc.next(Bump::Major).is_err());
    }
}
