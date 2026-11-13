//! Shared colors.

use div::prelude::*;
use div_components::current_theme;

/// The theme's colors as typed values, for dynamic `style:` attributes.
///
/// div-gap: 3. A dynamic style needs a typed value; theme tokens are CSS
/// text, so they are converted here once. Border colors take a
/// `StyleColor` where backgrounds take a `Color`, hence `border_line`.
#[derive(Clone, Copy)]
pub struct Colors {
    pub background: Color,
    pub foreground: Color,
    pub muted: Color,
    pub muted_foreground: Color,
    pub border: Color,
    pub border_line: StyleColor,
    pub sidebar: Color,
    pub sidebar_accent: Color,
}

pub fn colors() -> Colors {
    let theme = current_theme();
    Colors {
        background: theme.background.color(),
        foreground: theme.foreground.color(),
        muted: theme.muted.color(),
        muted_foreground: theme.muted_foreground.color(),
        border: theme.border.color(),
        border_line: StyleColor::Resolved(theme.border.color()),
        sidebar: theme.sidebar.color(),
        sidebar_accent: theme.sidebar_accent.color(),
    }
}
