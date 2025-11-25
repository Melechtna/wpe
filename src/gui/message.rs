use std::path::PathBuf;

use crate::config::WallpaperProfileEntry;
use crate::config::{ScaleMode, SlideshowOrder};
use crate::monitors::Monitor;

use super::{helpers::PathSelection, types::ThemePreference};

/// All events the iced state machine reacts to.
#[derive(Debug, Clone)]
pub(crate) enum Message {
    MonitorsLoaded(Result<Vec<Monitor>, String>),
    MonitorsUpdated(Vec<Monitor>),
    EntriesLoaded(Result<Vec<WallpaperProfileEntry>, String>),
    ThemeDetected(ThemePreference),
    SelectTab(usize),
    PathChanged(usize, String),
    BrowsePressed(usize, PathSelection),
    PathPicked(usize, Result<Option<PathBuf>, String>),
    EnabledToggled(usize, bool),
    ScaleChanged(usize, ScaleMode),
    OrderChanged(usize, SlideshowOrder),
    IntervalChanged(usize, String),
    StartPressed,
    StopPressed,
    Tick,
}
