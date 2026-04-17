/// Describes the persistence shape of a domain model.
///
/// Each model implements this once; the associated types tell the generic
/// repository methods what key, input, and filter types to accept.
pub trait TaxRecord: Sized + Send + Sync + 'static {
    /// Primary key type used by `get` / `delete`.
    type Key: Send + Sync;

    /// Input type accepted by `create`.  Often `Self`; for
    /// `[TaxEstimate](crate::models::TaxEstimate)` it is
    /// `[TaxEstimateInput](crate::models::TaxEstimateInput)`.
    type Draft: Send + Sync;

    /// Filtering criteria used by `list` / `delete_matching`.
    /// Use `()` when "list all" is the only meaningful query.
    type Filter: Send + Sync;
}
