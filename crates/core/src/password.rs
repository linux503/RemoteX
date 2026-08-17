use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::Path;

use crate::identity::generate_password;
use crate::{Error, Result};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct StoredPasswords {
    temp: String,
    permanent_hash: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PasswordVault {
    temp: String,
    permanent_hash: Option<String>,
}

impl PasswordVault {
    pub fn load(dir: &Path) -> Result<Self> {
        let path = password_path(dir);
        if path.exists() {
            let raw = std::fs::read_to_string(&path)?;
            let stored: StoredPasswords = serde_json::from_str(&raw)?;
            let temp = if is_valid_temp(&stored.temp) {
                stored.temp
            } else {
                generate_password()
            };
            return Ok(Self {
                temp,
                permanent_hash: stored.permanent_hash,
            });
        }
        Ok(Self::new())
    }

    pub fn save(&self, dir: &Path) -> Result<()> {
        std::fs::create_dir_all(dir)?;
        let stored = StoredPasswords {
            temp: self.temp.clone(),
            permanent_hash: self.permanent_hash.clone(),
        };
        std::fs::write(password_path(dir), serde_json::to_string_pretty(&stored)?)?;
        Ok(())
    }

    pub fn new() -> Self {
        Self {
            temp: generate_password(),
            permanent_hash: None,
        }
    }

    pub fn temp(&self) -> &str {
        &self.temp
    }

    pub fn refresh_temp(&mut self) {
        self.temp = generate_password();
    }

    pub fn set_temp(&mut self, password: &str) -> Result<()> {
        let normalized = normalize_temp(password);
        if !is_valid_temp(&normalized) {
            return Err(Error::Message(
                "Password must be 4–16 letters or numbers".into(),
            ));
        }
        self.temp = normalized;
        Ok(())
    }

    pub fn set_permanent(&mut self, password: &str) {
        if password.trim().is_empty() {
            self.permanent_hash = None;
        } else {
            self.permanent_hash = Some(hash_password(password));
        }
    }

    pub fn has_permanent(&self) -> bool {
        self.permanent_hash.is_some()
    }

    pub fn verify(&self, password: &str, unattended: bool) -> AuthOutcome {
        if unattended {
            if let Some(hash) = &self.permanent_hash {
                if hash_password(password) == *hash {
                    return AuthOutcome::Unattended;
                }
            }
        }
        if password.eq_ignore_ascii_case(&self.temp) {
            return AuthOutcome::NeedConfirm;
        }
        if let Some(hash) = &self.permanent_hash {
            if hash_password(password) == *hash {
                return AuthOutcome::NeedConfirm;
            }
        }
        AuthOutcome::Failed
    }
}

impl Default for PasswordVault {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthOutcome {
    Unattended,
    NeedConfirm,
    Failed,
}

pub fn password_path(dir: &Path) -> std::path::PathBuf {
    dir.join("password.json")
}

fn normalize_temp(password: &str) -> String {
    password
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_uppercase()
}

fn is_valid_temp(password: &str) -> bool {
    let len = password.len();
    (4..=16).contains(&len) && password.chars().all(|c| c.is_ascii_alphanumeric())
}

fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_custom_temp_password() {
        let mut vault = PasswordVault::new();
        vault.set_temp("abc123").unwrap();
        assert_eq!(vault.temp(), "ABC123");
        assert_eq!(vault.verify("abc123", false), AuthOutcome::NeedConfirm);
    }

    #[test]
    fn rejects_short_password() {
        let mut vault = PasswordVault::new();
        assert!(vault.set_temp("ab1").is_err());
    }
}
