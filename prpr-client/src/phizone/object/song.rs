use super::{MusicPosition, PZFile, PZObject, PZPointer, PZUser};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct PZSong {
    pub id: u64,
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
    // pub at_event: bool,
}
impl PZObject for PZSong {
    const QUERY_PATH: &'static str = "songs";

    fn id(&self) -> u64 {
        self.id
    }
}
