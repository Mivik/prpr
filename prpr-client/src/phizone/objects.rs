use super::Client;
use anyhow::Result;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use futures_util::Stream;
use serde::{de::DeserializeOwned, Deserialize};
use std::marker::PhantomData;

pub trait PZObject: DeserializeOwned {
    const QUERY_PATH: &'static str;
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "String")]
pub struct MusicPosition {
    pub seconds: u32,
}
impl TryFrom<String> for MusicPosition {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let seconds = || -> Option<u32> {
            let mut it = value.splitn(3, ':');
            let mut res = it.next()?.parse::<u32>().ok()?;
            res = res * 60 + it.next()?.parse::<u32>().ok()?;
            res = res * 60 + it.next()?.parse::<u32>().ok()?;
            Some(res)
        }()
        .ok_or("illegal position")?;
        Ok(MusicPosition { seconds })
    }
}

#[derive(Debug, Deserialize)]
#[serde(try_from = "u8")]
#[repr(u8)]
pub enum LevelType {
    EZ = 0,
    HD,
    IN,
    AT,
    SP,
}
impl TryFrom<u8> for LevelType {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use LevelType::*;
        Ok(match value {
            0 => EZ,
            1 => HD,
            2 => IN,
            3 => AT,
            4 => SP,
            x => {
                return Err(format!("illegal level type: {x}"));
            }
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(from = "usize")]
pub struct PZPointer<T: PZObject> {
    pub id: usize,
    _phantom: PhantomData<T>,
}
impl<T: PZObject> From<usize> for PZPointer<T> {
    fn from(value: usize) -> Self {
        Self {
            id: value,
            _phantom: PhantomData::default(),
        }
    }
}
impl<T: PZObject> Clone for PZPointer<T> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _phantom: PhantomData::default(),
        }
    }
}
impl<T: PZObject> PZPointer<T> {
    pub async fn fetch(&self) -> Result<T> {
        Client::fetch(self.id).await
    }
}

#[derive(Debug, Deserialize)]
#[serde(transparent)]
pub struct PZFile {
    url: String,
}
impl PZFile {
    pub async fn fetch(&self) -> Result<Bytes> {
        Ok(reqwest::get(&self.url).await?.bytes().await?)
    }

    pub async fn fetch_stream(&self) -> Result<impl Stream<Item = reqwest::Result<Bytes>>> {
        Ok(reqwest::get(&self.url).await?.bytes_stream())
    }
}

#[derive(Debug, Deserialize, Copy, Clone)]
#[repr(u8)]
#[serde(rename_all = "lowercase")]
pub enum PZUserRole {
    Banned = 0,
    Member,
    Qualified,
    Volunteer,
    Admin,
}

impl PZUserRole {
    pub fn priority(&self) -> u8 {
        *self as u8
    }
}

#[derive(Debug, Deserialize)]
pub struct PZUser {
    pub id: usize,
    pub username: String,
    pub avatar: PZFile,
    pub gender: u8,
    pub bio: String,
    #[serde(rename = "type")]
    pub role: PZUserRole,

    #[serde(rename = "following")]
    pub num_following: u32,
    #[serde(rename = "fans")]
    pub num_follower: u32,

    pub tag: Option<String>,
    pub exp: u32,
    pub rks: f32,

    pub language: String,
    #[serde(rename = "is_active")]
    pub active: bool,

    pub last_login: DateTime<Utc>,
    pub date_joined: DateTime<Utc>,
    pub date_of_birth: Option<String>,

    pub extra: Option<PZUserExtra>,
}
impl PZObject for PZUser {
    const QUERY_PATH: &'static str = "users";
}

#[derive(Debug, Deserialize)]
pub struct PZUserExtra {}

#[derive(Debug, Deserialize)]
pub struct PZSong {
    pub id: usize,
    pub name: String,
    pub composer: String,
    pub illustrator: String,
    pub uploader: PZPointer<PZUser>,
    pub description: String,

    pub bpm: String,
    pub offset: i32, // ?

    #[serde(rename = "song")]
    pub music: PZFile,
    pub illustration: PZFile,

    pub duration: MusicPosition,
    pub preview_start: MusicPosition,
    pub preview_end: MusicPosition,

    pub accessibility: u8,
    pub hidden: bool,

    pub time: DateTime<Utc>,

    #[serde(rename = "like_count")]
    pub num_count: u32,
    #[serde(rename = "comment_count")]
    pub num_comment: u32,
    #[serde(rename = "chapters")]
    pub num_chapters: u32,
    #[serde(rename = "charts")]
    pub num_charts: u32,

    pub at_event: bool,
}
impl PZObject for PZSong {
    const QUERY_PATH: &'static str = "songs";
}

#[derive(Debug, Deserialize)]
pub struct PZChartRating {
    #[serde(rename = "r_arrangement")]
    pub arrangement: f32,
    #[serde(rename = "r_feel")]
    pub feel: f32,
    #[serde(rename = "r_vfx")]
    pub vfx: f32,
    #[serde(rename = "r_innovativeness")]
    pub innovativeness: f32,
    #[serde(rename = "r_concord")]
    pub concord: f32,
    #[serde(rename = "r_impression")]
    pub impression: f32,
}
#[derive(Debug, Deserialize)]
pub struct PZChart {
    pub id: usize,
    pub song: PZPointer<PZSong>,
    pub charter: String,
    pub owner: PZPointer<PZUser>,
    pub level_type: LevelType,
    pub level: String,
    pub difficulty: f32,
    pub description: String,
    pub ranked: bool,
    #[serde(rename = "collab_status")]
    pub collab: bool,
    #[serde(rename = "rating")]
    pub rating_score: f32,
    #[serde(flatten)]
    pub rating: PZChartRating,

    pub chart: Option<PZFile>,

    pub time: DateTime<Utc>,

    #[serde(rename = "like_count")]
    pub num_like: u32,
    #[serde(rename = "score")]
    pub num_score: f32,
    #[serde(rename = "notes")]
    pub num_notes: u32,
    #[serde(rename = "comment_count")]
    pub num_comment: u32,
    #[serde(rename = "votes")]
    pub num_vote: u32,

    pub at_event: bool,
}
impl PZObject for PZChart {
    const QUERY_PATH: &'static str = "charts";
}
