
mod chart_order;
pub use chart_order::{ChartOrder, ChartOrderBox};

mod main;
pub use main::MainScene;

mod song;
pub use song::SongScene;

use anyhow::Result;
use macroquad::prelude::{Color, Rect};
use prpr::{
    fs::{self, FileSystem},
    scene::{NextScene, Scene},
    ui::{TextPainter, Ui},
};
use crate::dir;

pub fn fs_from_path(path: &str) -> Result<Box<dyn FileSystem>> {
    if let Some(name) = path.strip_prefix(':') {
        fs::fs_from_assets(format!("charts/{name}/"))
    } else {
        fs::fs_from_file(std::path::Path::new(&format!("{}/{}", dir::charts()?, path)))
    }
}
