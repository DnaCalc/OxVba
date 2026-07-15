//! Built-in `VBA.Collection`.
//!
//! The data model lives in `oxvba-runtime` (carried on the object box's `native_state`
//! slot) so vm2, vm3, and the JIT share ONE implementation; this module re-exports it so
//! the long-standing `oxvba_eval::collection::*` import path keeps resolving. The keyed
//! method *dispatch* shim ([`dispatch_collection`]) lives here in `oxvba-eval`, beside the
//! rest of the shared value/builtin kernel — it is the single place that turns a member +
//! `Variant` args into a `CollectionData` operation, so no VM re-implements it.

pub use oxvba_runtime::collection::{CollectionData, CollectionError, Selector};

use oxvba_runtime::Variant;

/// The omitted-argument sentinel (`DISP_E_PARAMNOTFOUND`) the VMs marshal an omitted optional
/// into; the shared dispatcher recognises it to tell "omitted" from a real value. (The VMs
/// carry their own copies of this stable HRESULT for marshalling.)
const MISSING_ARG: i32 = 0x8002_0004u32 as i32;

/// A built-in `Collection` member, resolved from the call-site member name/dispid by the VM.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectionMethod {
    Add,
    Item,
    Count,
    Remove,
}

/// The shared keyed-`Collection` dispatch: run `method` against `data` (the receiver's
/// box-owned [`CollectionData`], reached via `ObjectRef::with_native_collection`) with the
/// positional method `args` (omitted optionals arrive as the `MISSING_ARG` sentinel). Returns
/// the method result, or a [`CollectionError`] the caller maps to a VBA run-time error number
/// (9 / 457 / 5 / 449). This is the single implementation vm2, vm3, and the JIT all call.
pub fn dispatch_collection(
    method: CollectionMethod,
    data: &mut CollectionData,
    args: &[Variant],
) -> Result<Variant, CollectionError> {
    match method {
        CollectionMethod::Count => Ok(Variant::from_i32(data.count())),
        CollectionMethod::Add => {
            let value = args.first().cloned().unwrap_or_else(Variant::empty);
            let key = optional_key(args.get(1));
            let before = optional_selector(args.get(2));
            let after = optional_selector(args.get(3));
            data.add(value, key, before, after)?;
            Ok(Variant::empty())
        }
        CollectionMethod::Item => {
            let sel = required_selector(args.first())?;
            data.item(&sel)
        }
        CollectionMethod::Remove => {
            let sel = required_selector(args.first())?;
            data.remove(&sel)?;
            Ok(Variant::empty())
        }
    }
}

fn is_missing_arg(v: &Variant) -> bool {
    v.as_error_code() == Some(MISSING_ARG)
}

/// A selector from a Variant: a string is a (folded) key, anything else a 1-based index
/// (lenient numeric coercion; a non-numeric, non-string value yields index 0 → out of range).
fn variant_selector(v: &Variant) -> Selector {
    if let Some(s) = v.as_bstr() {
        Selector::Key(s.as_str().to_ascii_lowercase())
    } else {
        // Any numeric subtype is a 1-based Long index. Coerce through the shared
        // integer conversion (VBA banker's rounding) rather than a hand-rolled
        // as_i16/i32/i64/f64 chain that let Byte/SignedByte/Single/unsigned
        // subtypes fall through to index 0 (so `c.Item(CByte(1))` raised error 9).
        let index = crate::arith::int(v)
            .ok()
            .and_then(|n| i32::try_from(n).ok())
            .unwrap_or(0);
        Selector::Index(index)
    }
}

/// The folded string key from an `Add` key argument, or `None` if omitted.
fn optional_key(arg: Option<&Variant>) -> Option<String> {
    let v = arg?;
    (!is_missing_arg(v))
        .then(|| v.as_bstr().map(|s| s.as_str().to_ascii_lowercase()))
        .flatten()
}

/// A `Selector` from an optional `before`/`after` argument, or `None` if omitted.
fn optional_selector(arg: Option<&Variant>) -> Option<Selector> {
    let v = arg?;
    (!is_missing_arg(v)).then(|| variant_selector(v))
}

/// A required `Item`/`Remove` selector. Omitted → [`CollectionError::ArgNotOptional`] (449).
fn required_selector(arg: Option<&Variant>) -> Result<Selector, CollectionError> {
    match arg {
        Some(v) if !is_missing_arg(v) => Ok(variant_selector(v)),
        _ => Err(CollectionError::ArgNotOptional),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn count_add_item_remove_through_dispatch() {
        let mut c = CollectionData::default();
        dispatch_collection(CollectionMethod::Add, &mut c, &[Variant::from_i32(10)]).unwrap();
        dispatch_collection(CollectionMethod::Add, &mut c, &[Variant::from_i32(20)]).unwrap();
        assert_eq!(
            dispatch_collection(CollectionMethod::Count, &mut c, &[])
                .unwrap()
                .as_i32(),
            Some(2)
        );
        assert_eq!(
            dispatch_collection(CollectionMethod::Item, &mut c, &[Variant::from_i32(1)])
                .unwrap()
                .as_i32(),
            Some(10)
        );
        dispatch_collection(CollectionMethod::Remove, &mut c, &[Variant::from_i32(1)]).unwrap();
        assert_eq!(
            dispatch_collection(CollectionMethod::Item, &mut c, &[Variant::from_i32(1)])
                .unwrap()
                .as_i32(),
            Some(20)
        );
    }

    #[test]
    fn keyed_add_dup_missing_and_omitted_map_to_error_codes() {
        let mut c = CollectionData::default();
        dispatch_collection(
            CollectionMethod::Add,
            &mut c,
            &[Variant::from_i32(1), Variant::from_string("A")],
        )
        .unwrap();
        // Case-insensitive keyed Item → 1.
        assert_eq!(
            dispatch_collection(CollectionMethod::Item, &mut c, &[Variant::from_string("a")])
                .unwrap()
                .as_i32(),
            Some(1)
        );
        // Duplicate key → 457.
        assert_eq!(
            dispatch_collection(
                CollectionMethod::Add,
                &mut c,
                &[Variant::from_i32(2), Variant::from_string("a")]
            )
            .unwrap_err(),
            CollectionError::DuplicateKey
        );
        // Missing key → 9.
        assert_eq!(
            dispatch_collection(
                CollectionMethod::Item,
                &mut c,
                &[Variant::from_string("zz")]
            )
            .unwrap_err(),
            CollectionError::NotFound
        );
        // Omitted required selector → 449.
        assert_eq!(
            dispatch_collection(CollectionMethod::Item, &mut c, &[]).unwrap_err(),
            CollectionError::ArgNotOptional
        );
        // An explicit MISSING_ARG sentinel also reads as omitted → 449.
        assert_eq!(
            dispatch_collection(
                CollectionMethod::Item,
                &mut c,
                &[Variant::from_error_code(MISSING_ARG)]
            )
            .unwrap_err(),
            CollectionError::ArgNotOptional
        );
    }

    #[test]
    fn add_before_positions_the_insertion_via_dispatch() {
        let mut c = CollectionData::default();
        dispatch_collection(CollectionMethod::Add, &mut c, &[Variant::from_i32(10)]).unwrap();
        dispatch_collection(CollectionMethod::Add, &mut c, &[Variant::from_i32(30)]).unwrap();
        // Add 20 with key omitted (MISSING_ARG) and before=index 2 → [10, 20, 30].
        dispatch_collection(
            CollectionMethod::Add,
            &mut c,
            &[
                Variant::from_i32(20),
                Variant::from_error_code(MISSING_ARG),
                Variant::from_i32(2),
            ],
        )
        .unwrap();
        let vals: Vec<i32> = c.values().iter().filter_map(|v| v.as_i32()).collect();
        assert_eq!(vals, vec![10, 20, 30]);
    }

    #[test]
    fn integer_literal_selector_indexes_collection() {
        let mut c = CollectionData::default();
        dispatch_collection(CollectionMethod::Add, &mut c, &[Variant::from_i32(10)]).unwrap();
        dispatch_collection(CollectionMethod::Add, &mut c, &[Variant::from_i32(20)]).unwrap();
        assert_eq!(
            dispatch_collection(CollectionMethod::Item, &mut c, &[Variant::from_i16(2)])
                .unwrap()
                .as_i32(),
            Some(20)
        );
    }
}
