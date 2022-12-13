use serde::Deserialize;

#[derive(Deserialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub adjust_time: bool,
    pub aggressive: bool,
    pub aspect_ratio: Option<f32>,
    pub autoplay: bool,
    pub fix_aspect_ratio: bool,
    pub line_length: f32,
    pub multiple_hint: bool,
    pub offset: f32,
    pub particle: bool,
    pub speed: f64,
    pub volume_music: f64,
    pub volume_sfx: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            adjust_time: false,
            aggressive: true,
            aspect_ratio: None,
            autoplay: true,
            fix_aspect_ratio: false,
            line_length: 6.,
            multiple_hint: true,
            offset: 0.,
            particle: true,
            speed: 1.,
            volume_music: 1.,
            volume_sfx: 1.,
        }
    }
}
