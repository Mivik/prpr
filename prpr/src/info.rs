use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChartFormat {
    Rpe,
    Pec,
    Pgr,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct ChartInfo {
    pub id: Option<String>,

    pub name: String,
    pub difficulty: f32,
    pub level: String,
    pub charter: String,
    pub composer: String,
    pub illustrator: String,

    pub chart: String,
    pub format: Option<ChartFormat>,
    pub music: String,
    pub illustration: String,

    pub preview_time: f32,
    pub aspect_ratio: f32,
    pub line_length: f32,
    pub tip: Option<String>,

    pub intro: String,
    pub tags: Vec<String>,
}

impl Default for ChartInfo {
    fn default() -> Self {
        Self {
            id: None,

            name: "UK".to_string(),
            difficulty: 10.,
            level: "UK Lv.10".to_string(),
            charter: "UK".to_string(),
            composer: "UK".to_string(),
            illustrator: "UK".to_string(),

            chart: "chart.json".to_string(),
            format: None,
            music: "song.mp3".to_string(),
            illustration: "background.png".to_string(),

            preview_time: 0.,
            aspect_ratio: 16. / 9.,
            line_length: 6.,
            tip: None,

            intro: String::new(),
            tags: Vec::new(),
        }
    }
}
