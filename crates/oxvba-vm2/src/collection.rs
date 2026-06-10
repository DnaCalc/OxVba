//! Built-in `VBA.Collection` instance state.
//!
//! Each `New Collection` instance's ordered contents live VM-side, keyed by the
//! instance's `compat_identity` (see `Vm::collections`), rather than inside the
//! refcounted object box. Because a `Set c2 = c1` clones the `ObjectRef` (same
//! identity), both names map to the same entry — so VBA's reference semantics
//! fall out for free. Methods reach here by dispatching the class's native-bodied
//! procedures (`NativeMethodId::Collection*`).
//!
//! This is the data model only; mapping to VBA run-time error numbers (9 for a
//! bad index, …) happens at the VM dispatch site. Case-insensitive string keys
//! and `before`/`after` insertion are added in a following step.

use oxvba_runtime::Variant;

/// A VBA `Collection`'s ordered contents. Indices are 1-based at the VBA surface;
/// this type takes/returns 1-based indices and reports out-of-range via `Option`.
#[derive(Debug, Default, Clone)]
pub(crate) struct CollectionData {
    items: Vec<Variant>,
}

impl CollectionData {
    /// `Collection.Count`.
    pub(crate) fn count(&self) -> i32 {
        self.items.len() as i32
    }

    /// `Collection.Add item` (positional append).
    pub(crate) fn add(&mut self, value: Variant) {
        self.items.push(value);
    }

    /// `Collection.Item(index)` — 1-based. `None` ⇒ out of range (error 9).
    pub(crate) fn item_by_index(&self, index: i32) -> Option<Variant> {
        let zero_based = usize::try_from(index.checked_sub(1)?).ok()?;
        self.items.get(zero_based).cloned()
    }

    /// `Collection.Remove index` — 1-based. `false` ⇒ out of range (error 9).
    pub(crate) fn remove_by_index(&mut self, index: i32) -> bool {
        match usize::try_from(index.saturating_sub(1)) {
            Ok(i) if index >= 1 && i < self.items.len() => {
                self.items.remove(i);
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_add_and_positional_item() {
        let mut c = CollectionData::default();
        assert_eq!(c.count(), 0);
        c.add(Variant::from_i32(10));
        c.add(Variant::from_i32(20));
        assert_eq!(c.count(), 2);
        // 1-based.
        assert_eq!(c.item_by_index(1).and_then(|v| v.as_i32()), Some(10));
        assert_eq!(c.item_by_index(2).and_then(|v| v.as_i32()), Some(20));
    }

    #[test]
    fn item_out_of_range_is_none() {
        let mut c = CollectionData::default();
        c.add(Variant::from_i32(1));
        assert!(c.item_by_index(0).is_none(), "0 is below the 1-based floor");
        assert!(c.item_by_index(2).is_none(), "past the end");
        assert!(c.item_by_index(-1).is_none());
    }

    #[test]
    fn remove_shifts_following_indices() {
        let mut c = CollectionData::default();
        c.add(Variant::from_i32(10));
        c.add(Variant::from_i32(20));
        c.add(Variant::from_i32(30));
        assert!(c.remove_by_index(2)); // drop 20
        assert_eq!(c.count(), 2);
        assert_eq!(c.item_by_index(2).and_then(|v| v.as_i32()), Some(30));
        assert!(!c.remove_by_index(5), "out of range");
        assert!(!c.remove_by_index(0), "below 1-based floor");
    }
}
