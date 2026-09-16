use std::ops::Deref;
use std::sync::Arc;

use gpui::{App, Global};
use tax_core::TaxRepository;

/// Returned when something asks for the repository before it is installed.
#[derive(Debug, thiserror::Error)]
#[error("the database connection is not available")]
pub struct RepositoryUnavailable;

/// Process-wide database handle. Cheap to clone; holds an `Arc`.
///
/// Dereferences to [`TaxRepository`], so every repository method is reached
/// through the handle without restating the trait here.
#[derive(Clone)]
pub struct TaxRepo(Arc<dyn TaxRepository>);

impl Global for TaxRepo {}

impl TaxRepo {
    /// Wraps an existing repository implementation.
    pub fn new(repo: Arc<dyn TaxRepository>) -> Self {
        Self(repo)
    }

    /// Installs `repo` as the process-wide handle, replacing any earlier one.
    pub fn install(
        repo: Arc<dyn TaxRepository>,
        cx: &mut App,
    ) {
        cx.set_global(Self::new(repo));
    }

    /// The installed handle, or `None` before startup has finished.
    pub fn try_get(cx: &App) -> Option<Self> {
        cx.try_global::<Self>().cloned()
    }

    /// The installed handle, or [`RepositoryUnavailable`] so every caller
    /// reports the same message.
    pub fn require(cx: &App) -> Result<Self, RepositoryUnavailable> {
        Self::try_get(cx).ok_or(RepositoryUnavailable)
    }

    /// A clone of the inner `Arc`, for moving into an async task.
    pub fn arc(&self) -> Arc<dyn TaxRepository> {
        self.0.clone()
    }
}

impl Deref for TaxRepo {
    type Target = dyn TaxRepository;

    fn deref(&self) -> &Self::Target {
        &*self.0
    }
}
