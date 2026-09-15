use gpui::{Pixels, Size, px};

#[derive(Debug, Clone, Copy)]
pub struct WindowPreferences {
    pub size: Size<Pixels>,
}

impl Default for WindowPreferences {
    fn default() -> Self {
        Self {
            size: Size {
                width: px(820.0),
                height: px(900.0),
            },
        }
    }
}

impl WindowPreferences {
    pub fn new(
        width: impl Into<Pixels>,
        height: impl Into<Pixels>,
    ) -> Self {
        Self {
            size: Size {
                width: width.into(),
                height: height.into(),
            },
        }
    }
}
