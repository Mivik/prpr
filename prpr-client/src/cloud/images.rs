use super::LCFile;
use crate::dir;
use anyhow::{Context, Result};
use image::DynamicImage;
use std::path::Path;

pub struct Images;
impl Images {
    pub async fn load(file: &LCFile) -> Result<DynamicImage> {
        let path = format!("{}/{}", dir::cache_image()?, file.id);
        let path = Path::new(&path);
        Ok(if path.exists() {
            image::load_from_memory(&tokio::fs::read(path).await.context("Failed to read image")?)?
        } else {
            let bytes = reqwest::get(&file.url).await?.bytes().await?;
            tokio::fs::write(path, &bytes).await.context("Failed to save image")?;
            image::load_from_memory(&bytes)?
        })
    }
}
