/// Generate a deterministic UUID-shaped identifier for generated COM identities.
///
/// The algorithm is intentionally simple and stable, not cryptographic. It gives
/// reproducible LIBID/CLSID/IID values from the project and type names so v1 COM
/// projects do not need explicit GUID metadata before they can be built.
pub fn deterministic_uuid(namespace: &str, name: &str) -> String {
    const NAMESPACE_SEED: u128 = 0x4f58_5642_4100_0000_0000_0000_0000_0001;

    let input = format!("{namespace}\0{name}");
    let mut hash: u128 = NAMESPACE_SEED;
    for byte in input.bytes() {
        hash ^= u128::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3_0000_0000_0000_0000_001d);
    }

    let mut bytes = hash.to_be_bytes();
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    format!(
        "{:08X}-{:04X}-{:04X}-{:04X}-{:012X}",
        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        u16::from_be_bytes([bytes[4], bytes[5]]),
        u16::from_be_bytes([bytes[6], bytes[7]]),
        u16::from_be_bytes([bytes[8], bytes[9]]),
        u64::from_be_bytes([
            0, 0, bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
        ]),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_uuid_is_stable_and_uuid_shaped() {
        let first = deterministic_uuid("Demo", "Widget");
        let second = deterministic_uuid("Demo", "Widget");
        assert_eq!(first, second);
        assert_eq!(first.len(), 36);
        assert_eq!(first.as_bytes()[14], b'5');
        assert!(first.chars().all(|c| c.is_ascii_hexdigit() || c == '-'));
    }

    #[test]
    fn deterministic_uuid_distinguishes_names() {
        assert_ne!(
            deterministic_uuid("Demo", "Widget"),
            deterministic_uuid("Demo", "Other")
        );
        assert_ne!(
            deterministic_uuid("Demo", "Widget"),
            deterministic_uuid("Other", "Widget")
        );
    }
}
