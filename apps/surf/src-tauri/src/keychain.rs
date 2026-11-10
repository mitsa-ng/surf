//! macOS keychain-backed secret storage implementing surf-core's
//! SecretStore. Passwords are device-only accessible generic passwords.

#[cfg(target_os = "macos")]
pub struct KeychainStore {
    service: String,
}

#[cfg(target_os = "macos")]
impl KeychainStore {
    pub fn new(service: impl Into<String>) -> Self {
        Self { service: service.into() }
    }
}

#[cfg(target_os = "macos")]
impl surf_core::crypto::SecretStore for KeychainStore {
    fn put(&self, name: &str, value: &str) -> Result<(), surf_core::crypto::SecretError> {
        // Overwrite semantics: delete then set, so stale entries cannot linger.
        let _ = security_framework::passwords::delete_generic_password(&self.service, name);
        security_framework::passwords::set_generic_password(&self.service, name, value.as_bytes())
            .map_err(|e| surf_core::crypto::SecretError::Store(e.to_string()))
    }

    fn get(&self, name: &str) -> Result<String, surf_core::crypto::SecretError> {
        security_framework::passwords::get_generic_password(&self.service, name)
            .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
            .map_err(|e| match e.code() {
                // errSecItemNotFound
                -25300 => surf_core::crypto::SecretError::NotFound(name.to_owned()),
                _ => surf_core::crypto::SecretError::Store(e.to_string()),
            })
    }

    fn delete(&self, name: &str) -> Result<(), surf_core::crypto::SecretError> {
        security_framework::passwords::delete_generic_password(&self.service, name)
            .map_err(|e| match e.code() {
                -25300 => surf_core::crypto::SecretError::NotFound(name.to_owned()),
                _ => surf_core::crypto::SecretError::Store(e.to_string()),
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use surf_core::crypto::SecretStore;

    #[test]
    #[cfg(target_os = "macos")]
    fn keychain_roundtrip() {
        let store = KeychainStore::new("com.surf.browser.test");
        let name = format!("test-{}", std::process::id());
        store.put(&name, "v1").unwrap();
        assert_eq!(store.get(&name).unwrap(), "v1");
        store.put(&name, "v2").unwrap();
        assert_eq!(store.get(&name).unwrap(), "v2");
        store.delete(&name).unwrap();
        assert!(matches!(store.get(&name), Err(surf_core::crypto::SecretError::NotFound(_))));
    }
}
