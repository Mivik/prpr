use serde::{Deserialize, Serialize};

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChallengeModeColor {
    White,
    Green,
    Blue,
    Red,
    Golden,
    Rainbow,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(default)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub adjust_time: bool,
    pub aggressive: bool,
    pub aspect_ratio: Option<f32>,
    pub autoplay: bool,
    pub challenge_color: ChallengeModeColor,
    pub challenge_rank: u32,
    pub fix_aspect_ratio: bool,
    pub interactive: bool,
    pub multiple_hint: bool,
    pub note_scale: f32,
    pub offset: f32,
    pub particle: bool,
    pub player_name: String,
    pub player_rks: f32,
    pub speed: f32,
    pub tips: Vec<String>,
    pub volume_music: f32,
    pub volume_sfx: f32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            adjust_time: true,
            aggressive: true,
            aspect_ratio: None,
            autoplay: true,
            challenge_color: ChallengeModeColor::Golden,
            challenge_rank: 45,
            fix_aspect_ratio: false,
            interactive: true,
            multiple_hint: true,
            note_scale: 1.0,
            offset: 0.,
            particle: true,
            player_name: "Mivik".to_string(),
            player_rks: 15.,
            speed: 1.,
            tips: include_str!("tips.txt").split('\n').map(str::to_owned).collect(),
            volume_music: 1.,
            volume_sfx: 1.,
        }
    }
}
