use super::LCObject;
use crate::data::BriefChartInfo;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct LCFile {
    #[serde(rename = "objectId")]
    pub id: String,
    pub url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Pointer {
    #[serde(rename = "objectId")]
    pub id: String,
}

impl From<String> for Pointer {
    fn from(id: String) -> Self {
        Self { id }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(rename = "objectId")]
    pub id: String,
    #[serde(rename = "username")]
    pub name: String,
    pub session_token: Option<String>,
    pub avatar: Option<LCFile>,
    pub short_id: String,
    pub email: String,
    pub updated_at: DateTime<Utc>,
}

impl LCObject for User {
    const CLASS_NAME: &'static str = "_User";
}

#[derive(Clone, Deserialize)]
pub struct ChartItemData {
    #[serde(rename = "objectId")]
    pub id: String,

    #[serde(flatten)]
    pub info: BriefChartInfo,

    pub file: LCFile,
    pub illustration: LCFile,
}

impl LCObject for ChartItemData {
    const CLASS_NAME: &'static str = "Chart";
}
