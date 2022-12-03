use anyhow::Result;
use macroquad::{
    prelude::{warn, Camera2D, Mat4},
    text::{load_ttf_font, Font},
    texture::{load_image, Texture2D},
};

const FONT_PATH: &str = "font.ttf";

pub struct NoteStyle {
    pub click: Texture2D,
    pub hold_head: Texture2D,
    pub hold: Texture2D,
    pub hold_tail: Texture2D,
    pub flick: Texture2D,
    pub drag: Texture2D,
}

pub struct Resource {
    pub time: f32,
    pub camera: Camera2D,
    pub camera_matrix: Mat4,

    pub font: Font,
    pub note_style: NoteStyle,
    pub note_style_mh: NoteStyle,
}

impl Resource {
    pub async fn new() -> Result<Self> {
        async fn load_tex(path: &str) -> Result<Texture2D> {
            Ok(Texture2D::from_image(&load_image(path).await?))
        }
        let hold_tail = load_tex("hold_tail.png").await?;
        let note_style = NoteStyle {
            click: load_tex("click.png").await?,
            hold_head: load_tex("hold_head.png").await?,
            hold: load_tex("hold.png").await?,
            hold_tail,
            flick: load_tex("flick.png").await?,
            drag: load_tex("drag.png").await?,
        };
        Ok(Self {
            time: 0.0,
            camera: Camera2D::default(),
            camera_matrix: Mat4::default(),

            font: match load_ttf_font(FONT_PATH).await {
                Err(err) => {
                    warn!("Failed to load font from {FONT_PATH}, falling back to default\n{err:?}");
                    Font::default()
                }
                Ok(font) => font,
            },
            note_style,
            note_style_mh: NoteStyle {
                click: load_tex("click_mh.png").await?,
                hold_head: load_tex("hold_head_mh.png").await?,
                hold: load_tex("hold_mh.png").await?,
                hold_tail,
                flick: load_tex("flick_mh.png").await?,
                drag: load_tex("drag_mh.png").await?,
            },
        })
    }
}
