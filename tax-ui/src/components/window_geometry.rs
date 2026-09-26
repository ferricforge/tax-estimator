//! Window bookkeeping: converting between gpui rectangles and saved
//! geometry, saving a window's geometry, and remembering which windows are
//! open so their geometry can be saved when the application quits.

use gpui::{AnyWindowHandle, App, Bounds, Global, Pixels, Point, Size, Window, WindowBounds, px};

use crate::config::{AppConfig, WindowGeometry, on_screen_geometry};

/// Windows that are open, keyed by window name.
#[derive(Default)]
struct TrackedWindows(Vec<(&'static str, AnyWindowHandle)>);

impl Global for TrackedWindows {}

/// Converts a gpui rectangle into the plain geometry type.
fn bounds_to_geometry(bounds: Bounds<Pixels>) -> WindowGeometry {
    WindowGeometry {
        x: f32::from(bounds.origin.x),
        y: f32::from(bounds.origin.y),
        width: f32::from(bounds.size.width),
        height: f32::from(bounds.size.height),
    }
}

/// Converts a saved geometry back into a gpui rectangle.
fn geometry_to_bounds(geometry: WindowGeometry) -> Bounds<Pixels> {
    Bounds {
        origin: Point {
            x: px(geometry.x),
            y: px(geometry.y),
        },
        size: Size {
            width: px(geometry.width),
            height: px(geometry.height),
        },
    }
}

/// Returns the saved geometry for `window_id`, if one is stored and it still
/// fits entirely on a currently connected display.
///
/// Returns `None` when nothing is saved, or when the saved rectangle no
/// longer fits any display. The caller falls back to its own default
/// placement in that case.
pub fn restore_saved_window_bounds(
    window_id: &str,
    app_cx: &App,
) -> Option<Bounds<Pixels>> {
    let saved = AppConfig::get(app_cx).window_geometry.get(window_id)?;
    let displays: Vec<WindowGeometry> = app_cx
        .displays()
        .iter()
        .map(|display| bounds_to_geometry(display.bounds()))
        .collect();

    on_screen_geometry(saved, &displays).map(geometry_to_bounds)
}

/// Saves `window`'s current geometry under `window_id`, then persists the
/// configuration.
///
/// Only the window's normal (not maximized, minimized, or fullscreen) bounds
/// are meaningful to restore later. When the window is not in that state,
/// nothing is saved, and the previously stored geometry is left in place.
pub fn save_window_bounds(
    window_id: &str,
    window: &mut Window,
    cx: &mut App,
) {
    let WindowBounds::Windowed(bounds) = window.window_bounds() else {
        return;
    };

    let geometry = bounds_to_geometry(bounds);
    AppConfig::update(cx, |config| config.window_geometry.set(window_id, geometry));

    if let Err(error) = AppConfig::save(cx) {
        tracing::warn!(%error, window_id, "failed to save window geometry");
    }
}

/// Remembers `handle` as the open window named `window_id`, replacing any
/// handle remembered under that name.
pub fn track_window(
    window_id: &'static str,
    handle: AnyWindowHandle,
    cx: &mut App,
) {
    let tracked = cx.default_global::<TrackedWindows>();
    tracked.0.retain(|(id, _)| *id != window_id);
    tracked.0.push((window_id, handle));
}

/// The handle most recently tracked under `window_id`, if any.
pub fn tracked_window(
    window_id: &str,
    cx: &App,
) -> Option<AnyWindowHandle> {
    cx.try_global::<TrackedWindows>()?
        .0
        .iter()
        .find(|(id, _)| *id == window_id)
        .map(|(_, handle)| *handle)
}

/// Saves the geometry of every tracked window that is still open.
///
/// Called when the application quits, because quitting does not send each
/// window a close request.
pub fn save_tracked_window_bounds(cx: &mut App) {
    let windows = cx
        .try_global::<TrackedWindows>()
        .map(|tracked| tracked.0.clone())
        .unwrap_or_default();

    for (window_id, handle) in windows {
        let _ = handle.update(cx, |_, window, cx| {
            save_window_bounds(window_id, window, cx)
        });
    }
}
