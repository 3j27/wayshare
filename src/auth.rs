use std::path::Path;

use anyhow::{Context, Result};
use subtle::ConstantTimeEq;

pub trait Authenticator: Send + Sync {
    fn verify(&self, username: &str, password: &[u8]) -> bool;
}

pub struct StaticPassword {
    username: String,
    password: Vec<u8>,
}

impl StaticPassword {
    pub fn new(username: impl Into<String>, password: impl Into<Vec<u8>>) -> Self {
        Self {
            username: username.into(),
            password: password.into(),
        }
    }

    pub fn from_file(username: impl Into<String>, path: &Path) -> Result<Self> {
        let raw = std::fs::read(path)
            .with_context(|| format!("reading password file {}", path.display()))?;
        let end = raw
            .iter()
            .rposition(|&b| b != b'\n' && b != b'\r')
            .map_or(0, |i| i + 1);
        anyhow::ensure!(end > 0, "password file {} is empty", path.display());
        Ok(Self::new(username, &raw[..end]))
    }
}

impl Authenticator for StaticPassword {
    fn verify(&self, username: &str, password: &[u8]) -> bool {
        let user_ok = username.as_bytes().ct_eq(self.username.as_bytes());
        let pass_ok = password.ct_eq(&self.password);
        (user_ok & pass_ok).into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn static_password() {
        let a = StaticPassword::new("wes", "hunter2");
        assert!(a.verify("wes", b"hunter2"));
        assert!(!a.verify("wes", b"hunter3"));
        assert!(!a.verify("bob", b"hunter2"));
        assert!(!a.verify("wes", b""));
    }
}
