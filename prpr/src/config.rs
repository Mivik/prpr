use serde::Deserialize;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChartFormat {
    Rpe,
    Pec,
    Pgr,
}

#[derive(Deserialize)]
#[serde(default)]
#[serde(rename_all = "kebab-case")]
pub struct Config {
    pub id: String,

    pub title: String,
    pub level: String,
    pub charter: String,
    pub composer: String,
    pub illustrator: String,

    pub chart: String,
    pub format: Option<ChartFormat>,
    pub music: String,
    pub illustration: Option<String>,

    pub adjust_time: bool,
    pub aggressive: bool,
    pub aspect_ratio: f32,
    pub autoplay: bool,
    pub line_length: f32,
    pub particle: bool,
    pub speed: f64,
    pub volume_music: f64,
    pub volume_sfx: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            title: "UK".to_string(),
            level: "UK Lv.?".to_string(),
            charter: "UK".to_string(),
            composer: "UK".to_string(),
            illustrator: "UK".to_string(),

            chart: "chart.json".to_string(),
            format: None,
            music: "song.mp3".to_string(),
            illustration: None,

            adjust_time: true,
            aggressive: true,
            aspect_ratio: 16. / 9.,
            autoplay: true,
            line_length: 6.,
            particle: true,
            speed: 1.,
            volume_music: 1.,
            volume_sfx: 1.,
        }
    }
}
