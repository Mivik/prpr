use serde::Deserialize;
use super::LCObject;

#[derive(Clone, Deserialize)]
pub struct LCFile {
    pub url: String,
}

#[derive(Clone, Deserialize)]
pub struct ChartItemData {
    #[serde(rename = "objectId")]
    pub id: String,
    pub name: String,
    pub intro: String,
    pub tags: Vec<String>,

    pub file: LCFile,
    pub illustration: LCFile,
}

impl LCObject for ChartItemData {
    const CLASS_NAME: &'static str = "Chart";
}
