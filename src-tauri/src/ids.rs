/// Derives a safe, deterministic local ID from a namespace and a provider-controlled string.
///
/// Provider IDs must never be used directly as local IDs: they may contain path separators,
/// control characters, or other values that can cause collisions with locally generated IDs.
/// This function hashes the (namespace, provider_id) pair with FNV-1a and encodes the result
/// as hex, producing a compact, safe identifier that is purely alphanumeric.
pub fn deterministic_id(namespace: &str, provider_id: &str) -> String {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x00000100000001b3;
    let mut hash = OFFSET;
    for byte in namespace
        .bytes()
        .chain(b"\0".iter().copied())
        .chain(provider_id.bytes())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    format!("{namespace}-{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_and_safe() {
        let id = deterministic_id("codex-session", "../spoof");
        assert_eq!(id, deterministic_id("codex-session", "../spoof"));
        assert!(!id.contains(".."));
    }

    #[test]
    fn different_inputs_produce_different_ids() {
        assert_ne!(
            deterministic_id("session", "a"),
            deterministic_id("session", "b")
        );
        assert_ne!(
            deterministic_id("session", "x"),
            deterministic_id("other", "x")
        );
    }
}
