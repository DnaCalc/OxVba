//! Built-in `VBA.Collection` instance state.
//!
//! The data model lives here in `oxvba-runtime` so every consumer — vm2, vm3, and the
//! future JIT — shares ONE implementation. Each `New Collection` instance's ordered,
//! optionally string-keyed contents ride the refcounted object box itself (the
//! `native_state` slot on `CompatObjectBase`, reached via
//! [`crate::object_ref::ObjectRef::with_native_collection`]), not a VM-side map. Because a
//! `Set c2 = c1` clones the `ObjectRef` (same box), both names see the same
//! `CollectionData` — VBA reference semantics fall out for free — and box teardown
//! reclaims it, so there is no leak.
//!
//! This is the data model only; the shared dispatch site
//! ([`crate::collection`] has none — see `oxvba_eval::collection::dispatch_collection`)
//! maps [`CollectionError`] onto VBA run-time error numbers (9 / 457 / 5) and builds
//! [`Selector`]s from the call arguments. Keys are case-insensitive (folded to ASCII
//! lowercase).

use crate::Variant;

/// Selects an existing element (for `Item`/`Remove`) or an insertion anchor (for
/// `Add before`/`after`): a 1-based index, or a case-insensitive string key
/// (stored folded).
pub enum Selector {
    Index(i32),
    Key(String),
}

/// A collection operation failure, mapped to a VBA run-time error number at the
/// dispatch site.
#[derive(Debug, PartialEq, Eq)]
pub enum CollectionError {
    /// Index out of range / key not found → error 9.
    NotFound,
    /// `Add` with a key already present → error 457.
    DuplicateKey,
    /// `Add` given both `before` and `after` → error 5.
    BadArgument,
}

/// A VBA `Collection`'s ordered contents. Indices are 1-based at the VBA surface.
#[derive(Debug, Default, Clone)]
pub struct CollectionData {
    entries: Vec<Entry>,
}

#[derive(Debug, Clone)]
struct Entry {
    /// Folded (ASCII-lowercased) key, if the item was added with one.
    key: Option<String>,
    value: Variant,
}

impl CollectionData {
    /// `Collection.Count`.
    pub fn count(&self) -> i32 {
        self.entries.len() as i32
    }

    /// The values in insertion order — the `For Each` enumeration.
    pub fn values(&self) -> Vec<Variant> {
        self.entries.iter().map(|e| e.value.clone()).collect()
    }

    /// `Collection.Add item, [key], [before], [after]`. `key` is pre-folded. At
    /// most one of `before`/`after` may be given.
    pub fn add(
        &mut self,
        value: Variant,
        key: Option<String>,
        before: Option<Selector>,
        after: Option<Selector>,
    ) -> Result<(), CollectionError> {
        if before.is_some() && after.is_some() {
            return Err(CollectionError::BadArgument);
        }
        if let Some(k) = &key
            && self.position_of_key(k).is_some()
        {
            return Err(CollectionError::DuplicateKey);
        }
        let at = match (before, after) {
            (Some(sel), _) => self.resolve_index(&sel)?,
            (_, Some(sel)) => self.resolve_index(&sel)? + 1,
            _ => self.entries.len(),
        };
        self.entries.insert(at, Entry { key, value });
        Ok(())
    }

    /// `Collection.Item(indexOrKey)`.
    pub fn item(&self, sel: &Selector) -> Result<Variant, CollectionError> {
        let i = self.resolve_index(sel)?;
        Ok(self.entries[i].value.clone())
    }

    /// `Collection.Remove indexOrKey`.
    pub fn remove(&mut self, sel: &Selector) -> Result<(), CollectionError> {
        let i = self.resolve_index(sel)?;
        self.entries.remove(i);
        Ok(())
    }

    /// Zero-based position of `sel`, or [`CollectionError::NotFound`].
    fn resolve_index(&self, sel: &Selector) -> Result<usize, CollectionError> {
        match sel {
            Selector::Index(i) => {
                let zero = i
                    .checked_sub(1)
                    .and_then(|z| usize::try_from(z).ok())
                    .ok_or(CollectionError::NotFound)?;
                (zero < self.entries.len())
                    .then_some(zero)
                    .ok_or(CollectionError::NotFound)
            }
            Selector::Key(k) => self.position_of_key(k).ok_or(CollectionError::NotFound),
        }
    }

    fn position_of_key(&self, folded: &str) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| e.key.as_deref() == Some(folded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn i32_of(v: Result<Variant, CollectionError>) -> Option<i32> {
        v.ok().and_then(|v| v.as_i32())
    }

    #[test]
    fn positional_add_count_item_remove() {
        let mut c = CollectionData::default();
        assert_eq!(c.count(), 0);
        c.add(Variant::from_i32(10), None, None, None).unwrap();
        c.add(Variant::from_i32(20), None, None, None).unwrap();
        c.add(Variant::from_i32(30), None, None, None).unwrap();
        assert_eq!(c.count(), 3);
        assert_eq!(i32_of(c.item(&Selector::Index(1))), Some(10));
        assert_eq!(i32_of(c.item(&Selector::Index(3))), Some(30));
        assert_eq!(
            c.item(&Selector::Index(0)).unwrap_err(),
            CollectionError::NotFound
        );
        assert_eq!(
            c.item(&Selector::Index(4)).unwrap_err(),
            CollectionError::NotFound
        );
        c.remove(&Selector::Index(2)).unwrap(); // drop 20
        assert_eq!(c.count(), 2);
        assert_eq!(i32_of(c.item(&Selector::Index(2))), Some(30));
    }

    #[test]
    fn string_keys_are_case_insensitive_and_unique() {
        let mut c = CollectionData::default();
        c.add(Variant::from_i32(1), Some("alpha".into()), None, None)
            .unwrap();
        c.add(Variant::from_i32(2), Some("beta".into()), None, None)
            .unwrap();
        // Keyed lookup (the dispatch site folds the incoming key).
        assert_eq!(i32_of(c.item(&Selector::Key("alpha".into()))), Some(1));
        assert_eq!(i32_of(c.item(&Selector::Key("beta".into()))), Some(2));
        assert_eq!(
            c.item(&Selector::Key("missing".into())).unwrap_err(),
            CollectionError::NotFound
        );
        // Duplicate key → 457.
        assert_eq!(
            c.add(Variant::from_i32(9), Some("alpha".into()), None, None)
                .unwrap_err(),
            CollectionError::DuplicateKey
        );
        // Remove by key.
        c.remove(&Selector::Key("alpha".into())).unwrap();
        assert_eq!(c.count(), 1);
        assert_eq!(i32_of(c.item(&Selector::Index(1))), Some(2));
    }

    #[test]
    fn before_and_after_position_the_insertion() {
        let mut c = CollectionData::default();
        c.add(Variant::from_i32(10), None, None, None).unwrap();
        c.add(Variant::from_i32(30), None, None, None).unwrap();
        // Insert 20 before index 2 → [10, 20, 30].
        c.add(Variant::from_i32(20), None, Some(Selector::Index(2)), None)
            .unwrap();
        // Insert 5 before index 1 → [5, 10, 20, 30].
        c.add(Variant::from_i32(5), None, Some(Selector::Index(1)), None)
            .unwrap();
        // Insert 40 after the last → [5, 10, 20, 30, 40].
        c.add(Variant::from_i32(40), None, None, Some(Selector::Index(4)))
            .unwrap();
        let vals: Vec<i32> = c.values().iter().filter_map(|v| v.as_i32()).collect();
        assert_eq!(vals, vec![5, 10, 20, 30, 40]);
    }

    #[test]
    fn after_a_key_inserts_following_it() {
        let mut c = CollectionData::default();
        c.add(Variant::from_i32(1), Some("a".into()), None, None)
            .unwrap();
        c.add(Variant::from_i32(3), Some("c".into()), None, None)
            .unwrap();
        c.add(
            Variant::from_i32(2),
            Some("b".into()),
            None,
            Some(Selector::Key("a".into())),
        )
        .unwrap();
        let vals: Vec<i32> = c.values().iter().filter_map(|v| v.as_i32()).collect();
        assert_eq!(vals, vec![1, 2, 3]);
    }

    #[test]
    fn both_before_and_after_is_a_bad_argument() {
        let mut c = CollectionData::default();
        c.add(Variant::from_i32(1), None, None, None).unwrap();
        assert_eq!(
            c.add(
                Variant::from_i32(2),
                None,
                Some(Selector::Index(1)),
                Some(Selector::Index(1)),
            )
            .unwrap_err(),
            CollectionError::BadArgument
        );
    }
}
