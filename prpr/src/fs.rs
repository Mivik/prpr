use crate::{ext::thread_as_future, info::ChartInfo};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use concat_string::concat_string;
use macroquad::prelude::load_file;
use miniquad::warn;
use std::{
    fs,
    io::{Cursor, Read},
    path::PathBuf,
    sync::{Arc, Mutex},
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
        let path = self.0.join(path);
        thread_as_future(move || Ok(fs::read(path)?)).await
    }
}

struct ZipFileSystem(Arc<Mutex<ZipArchive<Cursor<Vec<u8>>>>>);

#[async_trait]
impl FileSystem for ZipFileSystem {
    async fn load_file(&mut self, path: &str) -> Result<Vec<u8>> {
        thread_as_future({
            let arc = Arc::clone(&self.0);
            let path = path.to_owned();
            move || {
                let mut zip = arc.lock().unwrap();
                let mut entry = zip.by_name(&path)?;
                let mut res = Vec::new();
                entry.read_to_end(&mut res)?;
                Ok(res)
            }
        })
        .await
    }
}

fn infer_diff(info: &mut ChartInfo, level: &str) {
    if let Ok(val) = level
        .chars()
        .rev()
        .take_while(|it| it.is_ascii_digit())
        .collect::<String>()
        .chars()
        .rev()
        .collect::<String>()
        .parse::<u32>()
    {
        info.difficulty = val as f32;
    }
}

fn info_from_kv<'a>(it: impl Iterator<Item = (&'a str, String)>) -> Result<ChartInfo> {
    let mut info = ChartInfo::default();
    for (key, value) in it {
        if key == "Path" {
            continue;
        }
        if key == "Level" {
            infer_diff(&mut info, &value);
        }
        if key == "AspectRatio" {
            info.aspect_ratio = value.parse().context("Failed to parse aspect ratio")?;
            continue;
        }
        if key == "NoteScale" || key == "ScaleRatio" {
            warn!("Note scale is ignored");
            continue;
        }
        if key == "GlobalAlpha" {
            warn!("Global alpha is ignored");
            continue;
        }
        if key == "BackgroundDim" {
            warn!("Background dim is ignored");
            continue;
        }
        *match key {
            "Name" => &mut info.name,
            "Music" | "Song" => &mut info.music,
            "Chart" => &mut info.chart,
            "Image" | "Picture" => &mut info.illustration,
            "Level" => &mut info.level,
            "Illustrator" => &mut info.illustrator,
            "Artist" | "Composer" | "Musician" => &mut info.composer,
            "Charter" | "Designer" => &mut info.charter,
            _ => bail!("Unknown key: {key}"),
        } = value;
    }
    Ok(info)
}

fn info_from_txt(text: &str) -> Result<ChartInfo> {
    let mut it = text.lines().peekable();
    let first = it.next();
    if first != Some("#") && first != Some("\u{feff}#") {
        bail!("Expected the first line to be #");
    }
    let kvs = it
        .map(|line| -> Result<(&str, String)> {
            let Some((key, value)) = line.split_once(": ") else {
            bail!("Expected \"Key: Value\"");
        };
            Ok((key, value.to_string()))
        })
        .collect::<Result<Vec<_>>>()?;
    info_from_kv(kvs.into_iter())
}

fn info_from_csv(bytes: Vec<u8>) -> Result<ChartInfo> {
    let mut reader = csv::Reader::from_reader(Cursor::new(bytes));
    // shitty design
    let headers = reader.headers()?.iter().map(str::to_owned).collect::<Vec<_>>();
    let records = reader.into_records().collect::<Vec<_>>();
    if records.len() != 1 {
        bail!("Expected exactly one record");
    }
    let record = records.into_iter().next().unwrap()?;
    info_from_kv(
        headers
            .iter()
            .zip(record.into_iter())
            .map(|(key, value)| (key.as_str(), value.to_owned())),
    )
}

pub async fn load_info(mut fs: Box<dyn FileSystem>) -> Result<(ChartInfo, Box<dyn FileSystem>)> {
    let info = if let Ok(bytes) = fs.load_file("info.yml").await {
        serde_yaml::from_str(&String::from_utf8(bytes)?)?
    } else if let Ok(bytes) = fs.load_file("info.txt").await {
        info_from_txt(&String::from_utf8(bytes)?)?
    } else if let Ok(bytes) = fs.load_file("info.csv").await {
        info_from_csv(bytes)?
    } else {
        bail!("None of info.yml, info.txt and info.csv is found");
    };
    Ok((info, fs))
}

pub fn fs_from_file(path: &str) -> Result<Box<dyn FileSystem>> {
    let meta = fs::metadata(path)?;
    Ok(if meta.is_file() {
        let bytes = fs::read(path).with_context(|| format!("Failed to read from {path}"))?;
        let zip = ZipArchive::new(Cursor::new(bytes)).with_context(|| format!("Cannot open {path} as zip archive"))?;
        Box::new(ZipFileSystem(Arc::new(Mutex::new(zip))))
    } else {
        Box::new(ExternalFileSystem(fs::canonicalize(path)?))
    })
}

pub fn fs_from_assets(name: &str) -> Result<Box<dyn FileSystem>> {
    if name.contains('/') {
        bail!("Illegal chart name: {name}");
    }
    Ok(Box::new(AssetsFileSystem(concat_string!("charts/", name, "/"))))
}
