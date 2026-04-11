#![allow(unused)]

/// Convert borrowed filter definitions into owned `String` values.
///
/// This is useful when filter data needs to be moved into an `async move`
/// closure or any `'static` context where references cannot be used.
pub fn owned_filters(filters: &[(&str, &[&str])]) -> Vec<(String, Vec<String>)> {
    filters
        .iter()
        .map(|(name, exts)| {
            (
                name.to_string(),
                exts.iter().map(|e| e.to_string()).collect(),
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_owned_filter_one_extension() {
        let input_filter = &[("Excel", &["xlsx"] as &[_])];
        let expected = vec![("Excel".to_string(), vec!["xlsx".to_string()])];
        let actual = owned_filters(input_filter);
        assert_eq!(
            actual, expected,
            "Results of owned_filters does not match expected"
        );
    }

    #[test]
    fn test_owned_filter_multiple_extensions() {
        let input_filter = &[("Excel", &["xlsx", "xlsm", "xlsb"] as &[_])];
        let expected = vec![(
            "Excel".to_string(),
            vec!["xlsx".to_string(), "xlsm".to_string(), "xlsb".to_string()],
        )];
        let actual = owned_filters(input_filter);
        assert_eq!(
            actual, expected,
            "Results of owned_filters does not match expected"
        );
    }

    #[test]
    fn test_owned_filter_multiple_types() {
        let input_filter = &[
            ("Excel", &["xlsx"] as &[_]),
            ("SQLite", &["db", "db3"] as &[_]),
        ];
        let expected = vec![
            ("Excel".to_string(), vec!["xlsx".to_string()]),
            (
                "SQLite".to_string(),
                vec!["db".to_string(), "db3".to_string()],
            ),
        ];
        let actual = owned_filters(input_filter);
        assert_eq!(
            actual, expected,
            "Results of owned_filters does not match expected"
        );
    }
}
