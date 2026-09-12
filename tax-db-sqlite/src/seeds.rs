//! Reference-data seed scripts, embedded into the binary at build time.
//!
//! `build.rs` scans the crate's `seeds/` directory and generates the `SEEDS`
//! table included below. Nothing is read from the filesystem at runtime, so a
//! fresh database can be seeded from any working directory — including a
//! packaged application with no source tree alongside it.

/// One seed script: its file name (used for ordering and diagnostics) and
/// the SQL it contains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seed {
    pub name: &'static str,
    pub sql: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/seeds.rs"));

/// Every embedded seed script, in file-name order.
pub fn embedded() -> &'static [Seed] {
    SEEDS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_seeds_are_present() {
        assert!(
            !embedded().is_empty(),
            "seeds/ should contain at least one .sql script"
        );
    }

    #[test]
    fn embedded_seeds_are_sorted_by_name() {
        let names: Vec<&str> = embedded().iter().map(|seed| seed.name).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();

        assert_eq!(names, sorted);
    }

    #[test]
    fn embedded_seeds_are_not_blank() {
        for seed in embedded() {
            assert!(!seed.sql.trim().is_empty(), "seed '{}' is empty", seed.name);
        }
    }
}
