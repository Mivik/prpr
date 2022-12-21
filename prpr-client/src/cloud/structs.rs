use super::LCObject;
use crate::data::BriefChartInfo;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
pub struct LCFile {
    pub url: String,
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
