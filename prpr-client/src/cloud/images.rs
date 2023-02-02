use super::LCFile;
use crate::dir;
use anyhow::{Context, Result};
use image::imageops::thumbnail;
use image::DynamicImage;
use std::future::Future;
use std::path::Path;

pub const THUMBNAIL_HEIGHT: u32 = 200;

pub struct Images;
impl Images {
    pub fn thumbnail(image: &DynamicImage) -> DynamicImage {
        let width = (image.width() as f32 / image.height() as f32 * THUMBNAIL_HEIGHT as f32).ceil() as u32;
        DynamicImage::ImageRgba8(thumbnail(image, width, THUMBNAIL_HEIGHT))
    }

    pub async fn load_lc(file: &LCFile) -> Result<DynamicImage> {
        Self::local_or_else(format!("{}/{}", dir::cache_image()?, file.id), async {
            let bytes = reqwest::get(&file.url).await?.bytes().await?;
            Ok(image::load_from_memory(&bytes)?)
        })
        .await
    }

    pub async fn load_lc_with_thumbnail(file: &LCFile) -> Result<(DynamicImage, DynamicImage)> {
        let image = Self::load_lc(file).await?;
        let thumbnail = Self::local_or_else(format!("{}/{}.thumb", dir::cache_image()?, file.id), async { Ok(Self::thumbnail(&image)) }).await?;
        Ok((thumbnail, image))
    }

    pub async fn local_or_else(path: impl AsRef<Path>, task: impl Future<Output = Result<DynamicImage>>) -> Result<DynamicImage> {
        let path = path.as_ref();
        Ok(if path.exists() {
            image::load_from_memory(&tokio::fs::read(path).await.context("Failed to read image")?)?
        } else {
            let image = task.await?;
            image.save_with_format(path, image::ImageFormat::Jpeg).context("Failed to save image")?;
            image
        })
    }
}
