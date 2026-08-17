use sha2::{Digest, Sha256};

use crate::identity::generate_password;

#[derive(Debug, Clone)]
pub struct PasswordVault {
    temp: String,
    permanent_hash: Option<String>,
}

impl PasswordVault {
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

fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    hex::encode(hasher.finalize())
}
