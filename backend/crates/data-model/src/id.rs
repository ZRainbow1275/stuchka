//! Object identifiers — UUID v7 (D9 §0.6, data/01 §1.0.1).
//!
//! All first-class object primary keys use `Uuid::now_v7()` (time-sortable, indexes
//! efficiently by creation order). ULID is abolished (master-index §0.2 D9).

use uuid::Uuid;

/// Generate a new time-ordered v7 UUID for an object primary key.
///
/// `law_ref` rows additionally carry a D8 URN `stable_id` string; the surrogate
/// `id` column is still a v7 UUID generated here.
#[inline]
pub fn new_id() -> Uuid {
    Uuid::now_v7()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_id_is_version_7() {
        let id = new_id();
        // UUID v7 has version nibble == 7.
        assert_eq!(id.get_version_num(), 7, "new_id must produce a v7 UUID");
    }

    #[test]
    fn new_id_is_time_monotonic() {
        // v7 embeds a millisecond timestamp in the high bits; ids generated in
        // sequence must be non-decreasing when compared as bytes (OM-02).
        let mut prev = new_id();
        for _ in 0..10_000 {
            let next = new_id();
            assert!(
                next >= prev,
                "v7 ids must be monotonically non-decreasing: {prev} then {next}"
            );
            prev = next;
        }
    }

    #[test]
    fn new_id_is_unique() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b, "consecutive ids must differ");
    }
}
