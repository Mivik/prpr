use crate::config::Config;
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use concat_string::concat_string;
use macroquad::prelude::load_file;
use std::{
    fs,
    io::{Cursor, Read},
    path::PathBuf,
};
use zip::ZipArchive;

#[async_trait]
pub trait FileSystem {
    async fn load_file(&mut self, path: &str) -> Result<Vec<u8>>;
}

struct AssetsFileSystem(String);

#[async_trait]
impl FileSystem for AssetsFileSystem {
    async fn load_file(&mut self, path: &str) -> Result<Vec<u8>> {
        Ok(load_file(&concat_string!(self.0, path)).await?)
    }
}

struct ExternalFileSystem(PathBuf);

#[async_trait]
impl FileSystem for ExternalFileSystem {
    async fn load_file(&mut self, path: &str) -> Result<Vec<u8>> {
        Ok(fs::read(self.0.join(path))?)
    }
}

struct ZipFileSystem(ZipArchive<Cursor<Vec<u8>>>);

#[async_trait]
impl FileSystem for ZipFileSystem {
    async fn load_file(&mut self, path: &str) -> Result<Vec<u8>> {
        let mut entry = self.0.by_name(path)?;
        let mut res = Vec::new();
        entry.read_to_end(&mut res)?;
        Ok(res)
    }
}

fn config_from_txt(text: &str) -> Result<Config> {
    let mut config = Config::default();
    let mut it = text.lines();
    if it.next() != Some("#") {
        bail!("Expected the first line to be #");
    }
    for line in it {
        let Some((key, value)) = line.split_once(": ") else {
            bail!("Expected \"Key: Value\"");
        };
        let value = value.to_string();
        match key {
            "Path" => {
                continue;
            }
            "Picture" => {
                config.illustration = Some(value);
                continue;
            }
            _ => {}
        }
        *match key {
            "Name" => &mut config.title,
            "Song" => &mut config.music,
            "Chart" => &mut config.chart,
            "Level" => &mut config.level,
            "Composer" => &mut config.composer,
            "Charter" => &mut config.charter,
            _ => bail!("Unknown key: {key}"),
        } = value;
    }
    Ok(config)
}

fn config_from_csv(bytes: Vec<u8>) -> Result<Config> {
    let mut config = Config::default();

    let mut reader = csv::Reader::from_reader(Cursor::new(bytes));
    // shitty design
    let headers = reader
        .headers()?
        .iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let records = reader.into_records().collect::<Vec<_>>();
    if records.len() != 1 {
        bail!("Expected exactly one record");
    }
    let record = records.into_iter().next().unwrap()?;
    for (key, value) in headers.into_iter().zip(record.into_iter()) {
        let value = value.to_string();
        if key == "Image" {
            config.illustration = Some(value);
            continue;
        }
        *match key.as_str() {
            "Name" => &mut config.title,
            "Music" => &mut config.music,
            "Chart" => &mut config.chart,
            "Level" => &mut config.level,
            "Composer" => &mut config.composer,
            "Designer" => &mut config.charter,
            _ => bail!("Unknown key: {key}"),
        } = value;
    }
    Ok(config)
}

pub async fn load_config(mut fs: Box<dyn FileSystem>) -> Result<(Config, Box<dyn FileSystem>)> {
    let config = if let Ok(bytes) = fs.load_file("info.yml").await {
        serde_yaml::from_str(&String::from_utf8(bytes)?)?
    } else if let Ok(bytes) = fs.load_file("info.txt").await {
        config_from_txt(&String::from_utf8(bytes)?)?
    } else if let Ok(bytes) = fs.load_file("info.csv").await {
        config_from_csv(bytes)?
    } else {
        bail!("None of info.yml, info.txt and info.csv is found");
    };
    Ok((config, fs))
}

pub fn fs_from_file(path: &str) -> Result<Box<dyn FileSystem>> {
    let meta = fs::metadata(path)?;
    Ok(if meta.is_file() {
        let bytes = fs::read(path).with_context(|| format!("Failed to read from {path}"))?;
        let zip = ZipArchive::new(Cursor::new(bytes))
            .with_context(|| format!("Cannot open {path} as zip archive"))?;
        Box::new(ZipFileSystem(zip))
    } else {
        Box::new(ExternalFileSystem(fs::canonicalize(path)?))
    })
}

pub fn fs_from_assets(name: &str) -> Result<Box<dyn FileSystem>> {
    if name.contains('/') {
        bail!("Illegal chart name: {name}");
    }
    Ok(Box::new(AssetsFileSystem(concat_string!(
        "charts/", name, "/"
    ))))
}
