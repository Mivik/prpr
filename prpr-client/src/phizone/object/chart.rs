use super::{LevelType, PZFile, PZObject, Ptr, PZRecord, PZSong, PZUser};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
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
#[derive(Clone, Debug, Deserialize)]
pub struct PZChart {
    pub id: u64,
    pub song: Ptr<PZSong>,
    pub charter: String,
    pub owner: Ptr<PZUser>,
    pub level_type: LevelType,
    pub level: String,
    pub difficulty: f32,
    pub description: Option<String>,
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

    pub records: Option<Vec<Ptr<PZRecord>>>,
    // pub at_event: bool,
}
impl PZObject for PZChart {
    const QUERY_PATH: &'static str = "charts";

    fn id(&self) -> u64 {
        self.id
    }
}
