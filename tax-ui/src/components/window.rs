// components

use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, Render, Styled, Subscription, Window, div,
};
use gpui_component::StyledExt;
use tracing::info;

#[cfg(not(target_os = "linux"))]
use crate::Quit;
#[cfg(not(target_os = "linux"))]
use crate::quit;

pub struct AppWindow {
    _window_close_subscription: Subscription,
    content: Option<Box<dyn Fn() -> AnyElement>>,
}

impl AppWindow {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let subscription = cx.on_window_closed(|_cx: &mut App| {
            info!("Window closed callback");
            #[cfg(not(target_os = "linux"))]
            quit(&Quit, _cx);
        });

        info!("Window constructed");
        Self {
            _window_close_subscription: subscription,
            content: None,
        }
    }

    /// Set a factory that produces the content to be rendered in the window.
    ///
    /// The factory is called on every render, ensuring stateless `RenderOnce`
    /// components like `Button` are reconstructed each frame.
    pub fn set_content(
        &mut self,
        content: impl Fn() -> AnyElement + 'static,
    ) {
        self.content = Some(Box::new(content));
    }
}

impl Render for AppWindow {
    fn render(
        &mut self,
        _: &mut Window,
        _cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let content = self.content.as_ref().map(|f| f());

        div()
            .v_flex()
            .gap_2()
            .size_full()
            .items_center()
            .justify_center()
            .children(content)
    }
}
