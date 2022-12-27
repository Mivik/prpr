use crate::{ext::spawn_task, info::ChartInfo};
use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use concat_string::concat_string;
use macroquad::prelude::load_file;
use miniquad::warn;
use std::{
    any::Any,
    collections::HashMap,
    fs,
    io::{Cursor, Read, Seek, Write},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use zip::{write::FileOptions, CompressionMethod, ZipArchive, ZipWriter};

pub fn update_zip<R: Read + Seek>(zip: &mut ZipArchive<R>, patches: HashMap<String, Vec<u8>>) -> Result<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut w = ZipWriter::new(Cursor::new(&mut buffer));
    let options = FileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o755);
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).unwrap();
        let path = match entry.enclosed_name() {
            Some(path) => path.to_owned(),
            None => continue,
        };
        let path = path.display().to_string();
        if entry.is_dir() {
            w.add_directory(path, options)?;
        } else if !patches.contains_key(&path) {
            w.start_file(&path, options)?;
            std::io::copy(&mut entry, &mut w)?;
        }
    }
    for (path, data) in patches.into_iter() {
        w.start_file(path, options)?;
        w.write_all(&data)?;
    }
    w.finish()?;
    drop(w);
    Ok(buffer)
}

#[async_trait]
pub trait FileSystem: Send {
    async fn load_file(&mut self, path: &str) -> Result<Vec<u8>>;
    fn clone_box(&mut self) -> Box<dyn FileSystem>;
    fn as_any(&mut self) -> &mut dyn Any;
}

#[derive(Clone)]
pub struct AssetsFileSystem(String);

#[async_trait]
impl FileSystem for AssetsFileSystem {
    async fn load_file(&mut self, path: &str) -> Result<Vec<u8>> {
        Ok(load_file(&concat_string!(self.0, path)).await?)
    }

    fn clone_box(&mut self) -> Box<dyn FileSystem> {
        Box::new(Self(self.0.clone()))
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Clone)]
pub struct ExternalFileSystem(PathBuf);

#[async_trait]
impl FileSystem for ExternalFileSystem {
    async fn load_file(&mut self, path: &str) -> Result<Vec<u8>> {
        let path = self.0.join(path);
        #[cfg(target_arch = "wasm32")]
        {
            unimplemented!("Cannot use external file system on wasm32")
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Ok(tokio::spawn(async move { tokio::fs::read(path).await }).await??)
        }
    }

    fn clone_box(&mut self) -> Box<dyn FileSystem> {
        Box::new(Self(self.0.clone()))
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

#[derive(Clone)]
pub struct ZipFileSystem(pub Arc<Mutex<ZipArchive<Cursor<Vec<u8>>>>>, String);

impl ZipFileSystem {
    pub fn new(bytes: Vec<u8>) -> Result<Self> {
        let zip = ZipArchive::new(Cursor::new(bytes))?;
        let root_dirs = zip
            .file_names()
            .filter(|it| it.ends_with('/') && it.find('/') == Some(it.len() - 1))
            .collect::<Vec<_>>();
        let root = if root_dirs.len() == 1 { root_dirs[0].to_owned() } else { String::new() };
        Ok(Self(Arc::new(Mutex::new(zip)), root))
    }
}

#[async_trait]
impl FileSystem for ZipFileSystem {
    async fn load_file(&mut self, path: &str) -> Result<Vec<u8>> {
        let arc = Arc::clone(&self.0);
        let path = concat_string!(self.1, path);
        spawn_task(async move {
            let mut zip = arc.lock().unwrap();
            let mut entry = zip.by_name(&path)?;
            let mut res = Vec::new();
            entry.read_to_end(&mut res)?;
            Ok(res)
        })
        .await?
    }

    fn clone_box(&mut self) -> Box<dyn FileSystem> {
        Box::new(Self(self.0.clone(), self.1.clone()))
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
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
    let record = reader.into_records().last().ok_or_else(|| anyhow!("Expected csv records"))??; // ??
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

pub fn fs_from_file(path: &Path) -> Result<Box<dyn FileSystem>> {
    let meta = fs::metadata(path)?;
    Ok(if meta.is_file() {
        let bytes = fs::read(path).with_context(|| format!("Failed to read from {}", path.display()))?;
        Box::new(ZipFileSystem::new(bytes).with_context(|| format!("Cannot open {} as zip archive", path.display()))?)
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
