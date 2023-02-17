use super::{PZChart, PZObject, Ptr, PZUser};
use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct PZRecord {
    pub id: u64,
    pub perfect: u32,
    pub good_early: u32,
    pub good_late: u32,
    pub bad: u32,
    pub miss: u32,

    pub score: u32,
    pub max_combo: u32,
    #[serde(rename = "acc")]
    pub accuracy: f64,

    pub full_combo: bool,

    pub rks: f64,
    pub perfect_judgment: u32,
    pub good_judgment: u32,
    pub time: DateTime<Utc>,
    pub chart: Ptr<PZChart>,
    pub player: Ptr<PZUser>,
    // pub event_part_id: Option<i64>,
    // pub app_id: Option<i64>,
    pub rank: Option<u32>,
}
impl PZObject for PZRecord {
    const QUERY_PATH: &'static str = "records";

    fn id(&self) -> u64 {
        self.id
    }
}
