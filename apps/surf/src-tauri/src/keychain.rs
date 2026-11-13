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

    fn list(&self, prefix: &str) -> Result<Vec<String>, surf_core::crypto::SecretError> {
        // One SecItem search for the whole service; accounts are few.
        use core_foundation::base::TCFType;
        use core_foundation::boolean::CFBoolean;
        use core_foundation::dictionary::CFDictionary;
        use core_foundation::string::CFString;
        use security_framework_sys::item::{
            kSecAttrAccount, kSecAttrService, kSecClass, kSecClassGenericPassword,
            kSecMatchLimit, kSecMatchLimitAll, kSecReturnAttributes,
        };
        use security_framework_sys::keychain_item::SecItemCopyMatching;

        let class = unsafe { CFString::wrap_under_get_rule(kSecClass) };
        let generic = unsafe { CFString::wrap_under_get_rule(kSecClassGenericPassword) };
        let service_key = unsafe { CFString::wrap_under_get_rule(kSecAttrService) };
        let account_key = unsafe { CFString::wrap_under_get_rule(kSecAttrAccount) };
        let return_attrs = unsafe { CFString::wrap_under_get_rule(kSecReturnAttributes) };
        let match_limit_key = unsafe { CFString::wrap_under_get_rule(kSecMatchLimit) };
        let match_all = unsafe { CFString::wrap_under_get_rule(kSecMatchLimitAll) };

        let query = CFDictionary::from_CFType_pairs(&[
            (class.into_CFType(), generic.into_CFType()),
            (service_key.into_CFType(), CFString::from(self.service.as_str()).into_CFType()),
            (return_attrs.into_CFType(), CFBoolean::true_value().into_CFType()),
            (match_limit_key.into_CFType(), match_all.into_CFType()),
        ]);
        let mut result: core_foundation_sys::base::CFTypeRef = std::ptr::null();
        let status = unsafe { SecItemCopyMatching(query.as_concrete_TypeRef(), &mut result) };
        if status == -25300 {
            return Ok(Vec::new()); // nothing stored yet
        }
        if status != 0 {
            return Err(surf_core::crypto::SecretError::Store(format!("SecItemCopyMatching {status}")));
        }
        let array = unsafe {
            core_foundation::array::CFArray::<core_foundation::base::CFType>::wrap_under_create_rule(result as _)
        };
        let mut names: Vec<String> = array
            .iter()
            .filter_map(|item| {
                let dict =
                    unsafe { CFDictionary::<CFString, core_foundation::base::CFType>::wrap_under_get_rule(item.as_CFTypeRef() as _) };
                let value = dict.find(&account_key)?;
                let account = unsafe { CFString::wrap_under_get_rule(value.as_CFTypeRef() as _) };
                Some(account.to_string())
            })
            .filter(|n| n.starts_with(prefix))
            .collect();
        names.sort();
        Ok(names)
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

    #[test]
    #[cfg(target_os = "macos")]
    fn keychain_lists_by_prefix() {
        let store = KeychainStore::new("com.surf.browser.test");
        let pid = std::process::id();
        let a = format!("pw:list-a-{pid}");
        let b = format!("pw:list-b-{pid}");
        let other = format!("list-other-{pid}");
        store.put(&a, "x").unwrap();
        store.put(&b, "y").unwrap();
        store.put(&other, "z").unwrap();
        let listed = store.list("pw:list-").unwrap();
        assert!(listed.contains(&a), "{listed:?}");
        assert!(listed.contains(&b), "{listed:?}");
        assert!(!listed.contains(&other), "{listed:?}");
        for n in [a, b, other] {
            store.delete(&n).unwrap();
        }
        assert!(!store.list("pw:list-").unwrap().iter().any(|n| n.contains(&pid.to_string())));
    }
}
