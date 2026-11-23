use ulid::Ulid;

use crate::AppError;

/// Creates a new sortable, unique identifier.
pub fn new_id() -> String {
    Ulid::new().to_string()
}

/// Returns true when `value` is a syntactically valid ULID.
pub fn is_valid_id(value: &str) -> bool {
    Ulid::from_string(value).is_ok()
}

/// Parses an id, returning a validation error suitable for the UI.
pub fn parse_id(value: &str) -> Result<Ulid, AppError> {
    Ulid::from_string(value).map_err(|_| AppError::validation("invalid server id"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_valid_and_unique() {
        let a = new_id();
        let b = new_id();
        assert_ne!(a, b);
        assert!(is_valid_id(&a));
    }

    #[test]
    fn ids_are_monotonic_when_sorted_as_strings() {
        let mut ids: Vec<String> = (0..16).map(|_| new_id()).collect();
        ids.sort();
        let parsed: Vec<Ulid> = ids.iter().map(|id| parse_id(id).unwrap()).collect();
        assert!(parsed.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[test]
    fn invalid_ids_are_rejected() {
        assert!(!is_valid_id("not-a-ulid"));
        assert!(parse_id("not-a-ulid").is_err());
    }
}
