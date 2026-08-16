use std::fmt;

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Long-lived credentials provisioned by a classic TRADFRI gateway.
///
/// The pre-shared key is deliberately redacted from `Debug`. Callers may
/// serialize this value for their own credential store, but Glimta does not
/// choose a storage location or persistence policy.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Credentials {
    identity: String,
    pre_shared_key: String,
}

impl Credentials {
    /// Construct credentials from an identity and pre-shared key.
    ///
    /// # Errors
    ///
    /// Returns an error if either value is empty.
    pub fn new(identity: impl Into<String>, pre_shared_key: impl Into<String>) -> Result<Self> {
        let identity = identity.into();
        let pre_shared_key = pre_shared_key.into();
        if identity.is_empty() {
            return Err(Error::EmptyCredential("identity"));
        }
        if pre_shared_key.is_empty() {
            return Err(Error::EmptyCredential("pre-shared key"));
        }
        Ok(Self {
            identity,
            pre_shared_key,
        })
    }

    #[must_use]
    pub fn identity(&self) -> &str {
        &self.identity
    }

    #[must_use]
    pub fn pre_shared_key(&self) -> &str {
        &self.pre_shared_key
    }
}

impl fmt::Debug for Credentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Credentials")
            .field("identity", &self.identity)
            .field("pre_shared_key", &"<redacted>")
            .finish()
    }
}
