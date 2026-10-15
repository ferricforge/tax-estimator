//! Windows that each show one form.
//!
//! A form window is an ordinary system window. It has the system title bar
//! and close button, can be moved anywhere on the desktop, and can be
//! resized. The form scrolls when the window is shorter than the form, and a
//! Close button sits below it. The main window stays usable while a form
//! window is open. The window's position and size are saved when it closes
//! and restored when it opens again.

use std::fmt;

use gpui::{
    AnyView, AnyWindowHandle, App, AppContext, Bounds, ClickEvent, Context, Entity, FocusHandle,
    Focusable, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Pixels, Render,
    ScrollHandle, Size, StatefulInteractiveElement, Styled, TitlebarOptions, Window, WindowBounds,
    WindowDecorations, WindowOptions, div, px,
};
use gpui_component::{ActiveTheme as _, Root, h_flex, scroll::Scrollbar, v_flex};

use crate::components::{
    make_button, restore_saved_window_bounds, save_window_bounds, track_window,
};

/// Smallest size a form window can be resized to.
const MIN_WINDOW_SIZE: Size<Pixels> = Size {
    width: px(360.0),
    height: px(240.0),
};

/// Space left between a new form window and the edges of its display.
const DISPLAY_MARGIN: Pixels = px(80.0);

/// Distance scrolled by one press of an arrow key.
const KEY_SCROLL_STEP: Pixels = px(40.0);

/// A form that can be shown in a window of its own.
pub trait WindowedForm: Render {
    /// Name under which the window's position and size are saved.
    const WINDOW_ID: &'static str;

    /// Text shown in the window's title bar.
    const WINDOW_TITLE: &'static str;

    /// Size of the window's content the first time it opens, before any
    /// geometry has been saved for it.
    const WINDOW_SIZE: Size<Pixels>;

    /// Rebuilds the parts of the form that belong to one window, such as its
    /// text inputs, so that they work in `window`. What the user has entered
    /// is kept.
    fn attach_to_window(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    );

    /// Called each time the form's window has opened and can show dialogs.
    /// A form overrides this to show a message that belongs with the form.
    fn window_opened(
        &mut self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        // Most forms have nothing to show.
    }
}

/// A form window that has been opened.
///
/// The value stays usable after the user closes the window; its methods then
/// do nothing.
#[derive(Clone, Copy)]
pub struct FormWindow {
    window_id: &'static str,
    handle: AnyWindowHandle,
}

impl FormWindow {
    /// Brings the window to the front. Returns `false` when the window has
    /// already been closed.
    pub fn activate(
        self,
        cx: &mut App,
    ) -> bool {
        let handle = self.handle;
        let activated = handle.update(cx, |_, window, _| window.activate_window());
        activated.is_ok()
    }

    /// Saves the window's geometry and closes it, if it is still open.
    pub fn close(
        self,
        cx: &mut App,
    ) {
        let window_id = self.window_id;
        // An error here only means the window is already closed.
        let _ = self.handle.update(cx, |_, window, cx| {
            save_window_bounds(window_id, window, cx);
            window.remove_window();
        });
    }
}

impl fmt::Debug for FormWindow {
    fn fmt(
        &self,
        f: &mut fmt::Formatter<'_>,
    ) -> fmt::Result {
        f.debug_struct("FormWindow")
            .field("window_id", &self.window_id)
            .finish()
    }
}

/// Opens a window that shows `form` under the system title bar.
///
/// The window takes the geometry saved under the form's `WINDOW_ID` when
/// there is one that still fits a display. Otherwise it opens centred on the
/// display that holds `opener`, no larger than that display. The form is
/// attached to the new window before it is shown, and told when the window
/// has opened.
pub fn open_form_window<F: WindowedForm>(
    form: Entity<F>,
    opener: &Window,
    cx: &mut App,
) -> anyhow::Result<FormWindow> {
    let bounds = restore_saved_window_bounds(F::WINDOW_ID, cx)
        .unwrap_or_else(|| default_window_bounds(F::WINDOW_SIZE, opener, cx));

    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: Some(TitlebarOptions {
            title: Some(F::WINDOW_TITLE.into()),
            ..Default::default()
        }),
        window_decorations: Some(WindowDecorations::Server),
        window_min_size: Some(MIN_WINDOW_SIZE),
        ..Default::default()
    };

    let opened_form = form.clone();
    let handle = cx.open_window(options, move |window, cx| {
        // The system close button goes through this hook. The Close button
        // and `FormWindow::close` remove the window directly and save the
        // geometry themselves.
        window.on_window_should_close(cx, |window, cx| {
            save_window_bounds(F::WINDOW_ID, window, cx);
            true
        });

        form.update(cx, |form, form_cx| {
            form.attach_to_window(window, form_cx);
        });
        let view = cx.new(|view_cx| {
            FormWindowView::new(F::WINDOW_ID, form.into(), window, view_cx)
        });
        cx.new(|root_cx| Root::new(AnyView::from(view), window, root_cx))
    })?;

    // Tracking lets the geometry be saved when the application quits, which
    // does not send the window a close request.
    let handle = AnyWindowHandle::from(handle);
    track_window(F::WINDOW_ID, handle, cx);

    // The window now has its dialog layer, so the form may show a dialog. An
    // error here only means the window was closed in the meantime.
    let _ = handle.update(cx, |_, window, cx| {
        opened_form.update(cx, |form, form_cx| {
            form.window_opened(window, form_cx);
        });
    });

    Ok(FormWindow {
        window_id: F::WINDOW_ID,
        handle,
    })
}

/// The rectangle for a form window that has no saved geometry: centred on
/// the display that holds `opener`, and no larger than that display.
fn default_window_bounds(
    preferred: Size<Pixels>,
    opener: &Window,
    cx: &App,
) -> Bounds<Pixels> {
    let display = opener.display(cx);
    let display_id = display.as_ref().map(|display| display.id());
    let window_size = display.map_or(preferred, |display| {
        let available = display.bounds().size;
        Size {
            width: preferred.width.min(available.width - DISPLAY_MARGIN),
            height: preferred.height.min(available.height - DISPLAY_MARGIN),
        }
    });

    Bounds::centered(display_id, window_size, cx)
}

/// The content of a form window: the form in a scrolling area, and a Close
/// button below it.
struct FormWindowView {
    window_id: &'static str,
    form: AnyView,
    scroll_handle: ScrollHandle,
    focus_handle: FocusHandle,
}

impl FormWindowView {
    fn new(
        window_id: &'static str,
        form: AnyView,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        // Focus the content so the arrow keys scroll the form before any
        // field has been clicked.
        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle);

        Self {
            window_id,
            form,
            scroll_handle: ScrollHandle::new(),
            focus_handle,
        }
    }

    fn on_close_clicked(
        &mut self,
        _event: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Removing the window does not run its close hook, so the geometry
        // is saved here.
        save_window_bounds(self.window_id, window, cx);
        window.remove_window();
    }

    /// Scrolls the form with the arrow and page keys. Runs for keys pressed
    /// anywhere in the window that the form's fields have not already used.
    fn on_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let page = self.scroll_handle.bounds().size.height;
        let distance = match event.keystroke.key.as_str() {
            "up" => -KEY_SCROLL_STEP,
            "down" => KEY_SCROLL_STEP,
            "pageup" => -page,
            "pagedown" => page,
            _ => return,
        };

        // gpui keeps the offset within the form's bounds on the next layout
        // pass, so it is not limited here.
        let mut offset = self.scroll_handle.offset();
        offset.y -= distance;
        self.scroll_handle.set_offset(offset);
        cx.notify();
        cx.stop_propagation();
    }
}

impl Render for FormWindowView {
    fn render(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        // The form area takes the height left over by the button row and
        // shrinks with the window, which is what makes the form scroll. The
        // scrollbar fills its parent, so it is placed in a layer pinned to
        // the edges of the form area.
        let form_area = div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(
                div()
                    .id("form")
                    .size_full()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll_handle)
                    .child(self.form.clone()),
            )
            .child(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .child(Scrollbar::vertical(&self.scroll_handle)),
            );

        let button_row = h_flex()
            .justify_end()
            .px_4()
            .py_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .child(make_button(
                "form-window-close",
                "Close",
                true,
                cx.listener(Self::on_close_clicked),
            ));

        // The layers let dialogs opened by the form, such as the
        // confirmation before clearing, appear in this window.
        v_flex()
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .size_full()
            .child(form_area)
            .child(button_row)
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}

impl Focusable for FormWindowView {
    fn focus_handle(
        &self,
        _cx: &App,
    ) -> FocusHandle {
        self.focus_handle.clone()
    }
}
