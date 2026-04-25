use std::{
    fmt::Display,
    ops::Deref,
    path::{Component, PathBuf},
    str::FromStr,
};

use serde::{Deserialize, Deserializer, Serialize};

use crate::error::Error;

#[derive(Debug, Clone, Serialize)]
pub struct SafePathBuf(PathBuf);

impl Deref for SafePathBuf {
    type Target = PathBuf;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SafePathBuf {
    pub fn new(path: PathBuf) -> Result<Self, Error> {
        if path.components().all(|c| matches!(c, Component::Normal(_))) {
            Ok(Self(path))
        } else {
            Err(Error::MaliciousPathComponent)
        }
    }
}

impl Display for SafePathBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.display())
    }
}

impl FromStr for SafePathBuf {
    type Err = Error;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        Self::new(s.into())
    }
}

impl<'de> Deserialize<'de> for SafePathBuf {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let path = PathBuf::deserialize(deserializer)?;
        SafePathBuf::new(path).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_unsafe_path() {
        assert!(SafePathBuf::from_str("..").is_err());
        assert!(SafePathBuf::from_str("../").is_err());
        assert!(SafePathBuf::from_str("/").is_err());
    }

    #[test]
    fn test_safe_path() {
        assert!(SafePathBuf::from_str("somerandomstring.txt").is_ok());
        assert!(SafePathBuf::from_str("这是一本书名.epub").is_ok());
        assert!(SafePathBuf::from_str("filename without extension").is_ok());
        assert!(SafePathBuf::from_str("filename-with-hyphen_and_underscore.md").is_ok());
    }
}
