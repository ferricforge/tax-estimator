use std::collections::BTreeSet;

use gpui::{App, IntoElement, ParentElement, SharedString, Styled, div};
use gpui_component::{ActiveTheme, v_flex};
use tax_core::validation::{Severity, ValidationIssue, ValidationReport};

/// Decides which fields show their messages.
///
/// A field shows its messages once the user has edited it. After a submit
/// attempt every field shows them, so a problem is never hidden behind a field
/// the user has not reached.
#[derive(Clone, Debug)]
pub struct FieldVisibility<K: Ord> {
    touched: BTreeSet<K>,
    submitted: bool,
}

impl<K: Ord> Default for FieldVisibility<K> {
    fn default() -> Self {
        Self {
            touched: BTreeSet::new(),
            submitted: false,
        }
    }
}

impl<K: Ord + Copy> FieldVisibility<K> {
    /// Records that the user has edited a field.
    pub fn touch(
        &mut self,
        field: K,
    ) {
        self.touched.insert(field);
    }

    /// Records a submit attempt, which shows every field's messages.
    pub fn submit(&mut self) {
        self.submitted = true;
    }

    /// Hides every message again, as after a clear.
    pub fn reset(&mut self) {
        self.touched.clear();
        self.submitted = false;
    }

    /// Returns `true` once a submit attempt has been made.
    pub fn is_submitted(&self) -> bool {
        self.submitted
    }

    /// Returns `true` when the field's messages should be shown.
    pub fn shows(
        &self,
        field: &K,
    ) -> bool {
        self.submitted || self.touched.contains(field)
    }
}

/// Returns the messages for `field` when its visibility allows them.
pub fn visible_issues<'a, K: Ord + Copy>(
    report: &'a ValidationReport<K>,
    visibility: &FieldVisibility<K>,
    field: K,
) -> &'a [ValidationIssue] {
    if visibility.shows(&field) {
        report.issues_for(&field)
    } else {
        &[]
    }
}

/// Renders the messages recorded for one field.
///
/// Returns [`None`] when the field has no messages so callers can feed the
/// result straight into `ParentElement::children`.
pub fn field_messages(
    issues: &[ValidationIssue],
    cx: &App,
) -> Option<impl IntoElement> {
    if issues.is_empty() {
        return None;
    }

    let rendered: Vec<_> = issues
        .iter()
        .map(|issue| {
            let color = match issue.severity {
                Severity::Error => cx.theme().danger,
                Severity::Warning => cx.theme().warning,
            };

            // The message is reference counted, so this shares the existing
            // allocation rather than copying the text.
            div()
                .text_xs()
                .text_color(color)
                .child(SharedString::from(issue.message.clone()))
        })
        .collect();

    Some(v_flex().gap_1().pl_2().children(rendered))
}

/// Renders a summary line shown near a form's action buttons.
pub fn form_error_banner(
    message: impl Into<SharedString>,
    cx: &App,
) -> impl IntoElement {
    div()
        .text_sm()
        .text_color(cx.theme().danger)
        .child(message.into())
}
