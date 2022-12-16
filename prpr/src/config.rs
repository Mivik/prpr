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
    pub interactive: bool,
    pub multiple_hint: bool,
    pub note_scale: f32,
    pub offset: f32,
    pub particle: bool,
    pub player_name: String,
    pub player_rks: f32,
    pub speed: f64,
    pub volume_music: f64,
    pub volume_sfx: f64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            adjust_time: true,
            aggressive: true,
            aspect_ratio: None,
            autoplay: true,
            fix_aspect_ratio: false,
            interactive: true,
            multiple_hint: true,
            note_scale: 1.0,
            offset: 0.,
            particle: true,
            player_name: "Mivik".to_string(),
            player_rks: 15.,
            speed: 1.,
            volume_music: 1.,
            volume_sfx: 1.,
        }
    }
}
