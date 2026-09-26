//! Remembered window positions and sizes.
//!
//! These types hold plain values. Converting them to and from a file or a
//! platform store is the job of the config store.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Key of the main application window.
pub const MAIN_WINDOW: &str = "main";

/// Key of the preferences window.
pub const PREFERENCES_WINDOW: &str = "preferences";

/// Smallest width a window may be restored to, in logical pixels.
const MIN_WIDTH: f32 = 320.0;

/// Smallest height a window may be restored to, in logical pixels.
const MIN_HEIGHT: f32 = 240.0;

/// Position and size of a window in logical pixels, taken while the window
/// was in its normal (not maximized or minimized) state.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowGeometry {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// Width and height of a window in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowSize {
    pub width: f32,
    pub height: f32,
}

impl WindowGeometry {
    /// Whether this rectangle lies entirely inside `display`.
    fn fits_within(
        &self,
        display: &WindowGeometry,
    ) -> bool {
        self.x >= display.x
            && self.y >= display.y
            && self.x + self.width <= display.x + display.width
            && self.y + self.height <= display.y + display.height
    }

    /// This geometry with its size raised to the minimum.
    fn with_minimum_size(self) -> Self {
        Self {
            width: self.width.max(MIN_WIDTH),
            height: self.height.max(MIN_HEIGHT),
            ..self
        }
    }

    /// A rectangle of `size` placed at the center of `display`.
    fn centered_on(
        display: &WindowGeometry,
        size: WindowSize,
    ) -> Self {
        Self {
            x: display.x + (display.width - size.width) / 2.0,
            y: display.y + (display.height - size.height) / 2.0,
            width: size.width,
            height: size.height,
        }
    }
}

/// Remembered geometry for every window, keyed by window name.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WindowGeometries {
    entries: BTreeMap<String, WindowGeometry>,
}

impl WindowGeometries {
    /// Returns the geometry saved for `window`, if any.
    pub fn get(
        &self,
        window: &str,
    ) -> Option<WindowGeometry> {
        self.entries.get(window).copied()
    }

    /// Replaces the geometry saved for `window`.
    pub fn set(
        &mut self,
        window: &str,
        geometry: WindowGeometry,
    ) {
        self.entries.insert(window.to_string(), geometry);
    }
}

/// Raises `saved` to the minimum size and returns it when the result fits
/// entirely inside one of `displays`. Returns `None` otherwise.
pub fn on_screen_geometry(
    saved: WindowGeometry,
    displays: &[WindowGeometry],
) -> Option<WindowGeometry> {
    let candidate = saved.with_minimum_size();

    displays
        .iter()
        .any(|display| candidate.fits_within(display))
        .then_some(candidate)
}

/// Chooses where a window opens.
///
/// `saved` is used, raised to the minimum size, when it fits entirely inside
/// one of `displays`. Otherwise the window is centered on the first display
/// (the primary display) at `default_size`.
///
/// Returns `None` when `displays` is empty.
pub fn resolve_geometry(
    saved: Option<WindowGeometry>,
    default_size: WindowSize,
    displays: &[WindowGeometry],
) -> Option<WindowGeometry> {
    let primary = displays.first()?;
    let fallback = WindowGeometry::centered_on(primary, default_size);
    let on_screen = saved.and_then(|saved| on_screen_geometry(saved, displays));

    Some(on_screen.unwrap_or(fallback))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::{
        MAIN_WINDOW, PREFERENCES_WINDOW, WindowGeometries, WindowGeometry, WindowSize,
        on_screen_geometry, resolve_geometry,
    };

    const PRIMARY: WindowGeometry = WindowGeometry {
        x: 0.0,
        y: 0.0,
        width: 1000.0,
        height: 800.0,
    };

    const SECONDARY: WindowGeometry = WindowGeometry {
        x: 1000.0,
        y: 0.0,
        width: 1000.0,
        height: 800.0,
    };

    const LEFT: WindowGeometry = WindowGeometry {
        x: -1000.0,
        y: 0.0,
        width: 1000.0,
        height: 800.0,
    };

    const GEOMETRY: WindowGeometry = WindowGeometry {
        x: 120.0,
        y: 80.0,
        width: 900.0,
        height: 700.0,
    };

    const DEFAULT_SIZE: WindowSize = WindowSize {
        width: 600.0,
        height: 400.0,
    };

    #[test]
    fn missing_window_returns_none() {
        let geometries = WindowGeometries::default();

        assert_eq!(geometries.get(MAIN_WINDOW), None);
    }

    #[test]
    fn set_then_get_returns_the_same_geometry() {
        let mut geometries = WindowGeometries::default();

        geometries.set(MAIN_WINDOW, GEOMETRY);

        assert_eq!(geometries.get(MAIN_WINDOW), Some(GEOMETRY));
    }

    #[test]
    fn set_replaces_the_geometry_for_the_same_window() {
        let mut geometries = WindowGeometries::default();
        let moved = WindowGeometry { x: 5.0, ..GEOMETRY };

        geometries.set(MAIN_WINDOW, GEOMETRY);
        geometries.set(MAIN_WINDOW, moved);

        assert_eq!(geometries.get(MAIN_WINDOW), Some(moved));
    }

    #[test]
    fn windows_are_stored_separately() {
        let mut geometries = WindowGeometries::default();

        geometries.set(MAIN_WINDOW, GEOMETRY);

        assert_eq!(geometries.get(PREFERENCES_WINDOW), None);
    }

    #[test]
    fn on_screen_geometry_returns_the_geometry_when_it_fits() {
        let saved = WindowGeometry {
            x: 100.0,
            y: 100.0,
            width: 600.0,
            height: 400.0,
        };

        assert_eq!(on_screen_geometry(saved, &[PRIMARY]), Some(saved));
    }

    #[test]
    fn on_screen_geometry_returns_none_when_it_does_not_fit_any_display() {
        let saved = WindowGeometry {
            x: 800.0,
            y: 0.0,
            width: 400.0,
            height: 300.0,
        };

        assert_eq!(on_screen_geometry(saved, &[PRIMARY]), None);
    }

    #[test]
    fn saved_geometry_inside_the_primary_display_is_used() {
        let saved = WindowGeometry {
            x: 100.0,
            y: 100.0,
            width: 600.0,
            height: 400.0,
        };

        let actual = resolve_geometry(Some(saved), DEFAULT_SIZE, &[PRIMARY]);

        assert_eq!(actual, Some(saved));
    }

    #[test]
    fn saved_geometry_on_a_secondary_display_is_used() {
        let saved = WindowGeometry {
            x: 1100.0,
            y: 50.0,
            width: 600.0,
            height: 400.0,
        };

        let actual = resolve_geometry(Some(saved), DEFAULT_SIZE, &[PRIMARY, SECONDARY]);

        assert_eq!(actual, Some(saved));
    }

    #[test]
    fn saved_geometry_on_a_display_with_negative_origin_is_used() {
        let saved = WindowGeometry {
            x: -800.0,
            y: 100.0,
            width: 600.0,
            height: 400.0,
        };

        let actual = resolve_geometry(Some(saved), DEFAULT_SIZE, &[PRIMARY, LEFT]);

        assert_eq!(actual, Some(saved));
    }

    #[test]
    fn saved_geometry_partly_off_screen_falls_back_to_centered_default() {
        let saved = WindowGeometry {
            x: 800.0,
            y: 0.0,
            width: 400.0,
            height: 300.0,
        };
        let expected = WindowGeometry {
            x: 200.0,
            y: 200.0,
            width: 600.0,
            height: 400.0,
        };

        let actual = resolve_geometry(Some(saved), DEFAULT_SIZE, &[PRIMARY]);

        assert_eq!(actual, Some(expected));
    }

    #[test]
    fn saved_geometry_smaller_than_the_minimum_is_raised_to_it() {
        let saved = WindowGeometry {
            x: 10.0,
            y: 10.0,
            width: 100.0,
            height: 100.0,
        };
        let expected = WindowGeometry {
            x: 10.0,
            y: 10.0,
            width: 320.0,
            height: 240.0,
        };

        let actual = resolve_geometry(Some(saved), DEFAULT_SIZE, &[PRIMARY]);

        assert_eq!(actual, Some(expected));
    }

    #[test]
    fn missing_geometry_centers_the_default_on_the_primary_display() {
        let expected = WindowGeometry {
            x: 200.0,
            y: 200.0,
            width: 600.0,
            height: 400.0,
        };

        let actual = resolve_geometry(None, DEFAULT_SIZE, &[PRIMARY, SECONDARY]);

        assert_eq!(actual, Some(expected));
    }

    #[test]
    fn no_displays_returns_none() {
        let actual = resolve_geometry(Some(GEOMETRY), DEFAULT_SIZE, &[]);

        assert_eq!(actual, None);
    }
}
