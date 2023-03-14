mod chart_order;
use std::{io::Read, sync::Arc};

use cap_std::ambient_authority;
pub use chart_order::{ChartOrder, ChartOrderBox};

mod main;
pub use main::MainScene;

mod song;
pub use song::SongScene;

use crate::dir;
use anyhow::Result;
use macroquad::prelude::{Color, Rect};
use prpr::{
    fs::{self, FileSystem},
    scene::{NextScene, Scene},
    ui::{TextPainter, Ui},
};

pub fn fs_from_path(path: &str) -> Result<Box<dyn FileSystem>> {
    if let Some(name) = path.strip_prefix(':') {
        fs::fs_from_assets(format!("charts/{name}/"))
    } else {
        let full_path = format!("{}/{}", dir::charts()?, path);
        if path.starts_with("download/") {
            let dir = Arc::new(cap_std::fs::Dir::open_ambient_dir(full_path, ambient_authority())?);
            let mut song = String::new();
            dir.open("song")?.read_to_string(&mut song)?;
            Ok(Box::new(fs::PZFileSystem(dir, Arc::new(format!("{}/{song}", dir::songs()?)))))
        } else {
            fs::fs_from_file(std::path::Path::new(&full_path))
        }
    }
}
