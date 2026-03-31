use crate::{TypeLibCacheScope, TypeLibMetadataBlob, TypeLibResolvedIdentity};
use std::collections::BTreeMap;

#[derive(Debug, Default)]
pub struct TypeLibMetadataCacheState {
    pub metadata: BTreeMap<String, TypeLibMetadataBlob>,
}

impl TypeLibMetadataCacheState {
    pub fn load_or_build(
        &mut self,
        identity: &TypeLibResolvedIdentity,
        build: impl FnOnce(&TypeLibResolvedIdentity) -> TypeLibMetadataBlob,
    ) -> TypeLibMetadataBlob {
        if let Some(cached) = self.metadata.get(&identity.cache_key) {
            return cached.clone();
        }
        let blob = build(identity);
        self.metadata
            .insert(identity.cache_key.clone(), blob.clone());
        blob
    }

    pub fn invalidate(
        &mut self,
        scope: TypeLibCacheScope,
        reference_name: Option<&str>,
    ) -> Result<usize, &'static str> {
        Ok(match scope {
            TypeLibCacheScope::Global => {
                let count = self.metadata.len();
                self.metadata.clear();
                count
            }
            TypeLibCacheScope::Reference => {
                let Some(reference_name) = reference_name else {
                    return Err("reference scope requires reference_name");
                };
                let key = normalize_ci_token(reference_name);
                let before = self.metadata.len();
                self.metadata
                    .retain(|_, blob| normalize_ci_token(&blob.identity.reference_name) != key);
                before.saturating_sub(self.metadata.len())
            }
        })
    }
}

fn normalize_ci_token(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::TypeLibMetadataCacheState;
    use crate::{TypeLibCacheScope, TypeLibMetadataBlob, TypeLibResolvedIdentity};

    fn sample_identity(reference_name: &str, cache_key: &str) -> TypeLibResolvedIdentity {
        TypeLibResolvedIdentity {
            reference_name: reference_name.to_string(),
            requested_coclass: None,
            importlib: "stdole2.tlb".to_string(),
            libid: Some("{00020430-0000-0000-C000-000000000046}".to_string()),
            major_version: 2,
            minor_version: 0,
            lcid: Some(0),
            cache_key: cache_key.to_string(),
        }
    }

    fn sample_blob(reference_name: &str, cache_key: &str) -> TypeLibMetadataBlob {
        TypeLibMetadataBlob {
            identity: sample_identity(reference_name, cache_key),
            activation_prog_id: None,
            member_name_to_token: Vec::new(),
            members: Vec::new(),
            events: Vec::new(),
        }
    }

    #[test]
    fn load_or_build_uses_cached_entry() {
        let identity = sample_identity("StdOle", "stdole#2.0");
        let mut cache = TypeLibMetadataCacheState::default();
        let first = cache.load_or_build(&identity, |_| sample_blob("StdOle", "stdole#2.0"));
        let second = cache.load_or_build(&identity, |_| sample_blob("Wrong", "wrong"));
        assert_eq!(first.identity.cache_key, "stdole#2.0");
        assert_eq!(second.identity.reference_name, "StdOle");
    }

    #[test]
    fn invalidate_reference_is_case_insensitive() {
        let mut cache = TypeLibMetadataCacheState::default();
        cache
            .metadata
            .insert("a".to_string(), sample_blob("StdOle", "a"));
        cache
            .metadata
            .insert("b".to_string(), sample_blob("Other", "b"));

        let removed = cache
            .invalidate(TypeLibCacheScope::Reference, Some(" stDOLE "))
            .expect("reference invalidation should succeed");
        assert_eq!(removed, 1);
        assert_eq!(cache.metadata.len(), 1);
        assert!(cache.metadata.contains_key("b"));
    }
}
