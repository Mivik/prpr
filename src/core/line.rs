use super::{Anim, AnimFloat, Matrix, Note, Object, Resource, ScopedTransform, Vector};
use macroquad::prelude::vec2;
use macroquad::text::{draw_text_ex, measure_text, TextParams};
use macroquad::{
    shapes::draw_line,
    texture::{draw_texture_ex, DrawTextureParams, Texture2D},
};

#[derive(Default)]
pub enum JudgeLineKind {
    #[default]
    Normal,
    Texture(Texture2D),
    Text(Anim<String>),
}

#[derive(Default)]
pub struct JudgeLine {
    pub object: Object,
    pub kind: JudgeLineKind,
    pub height: AnimFloat,
    pub notes_above: Vec<Note>,
    pub notes_below: Vec<Note>,
}

impl JudgeLine {
    pub fn set_time(&mut self, time: f32) {
        self.object.set_time(time);
        if let JudgeLineKind::Text(anim) = &mut self.kind {
            anim.set_time(time);
        }
        self.height.set_time(time);
        for note in &mut self.notes_above {
            note.set_time(time);
        }
        for note in &mut self.notes_below {
            note.set_time(time);
        }
    }

    pub fn render(&self, res: &Resource) {
        let tr = self.object.now();
        tr.apply_render(|| {
            self.object.now_scale().apply_render(|| match &self.kind {
                JudgeLineKind::Normal => {
                    draw_line(-6.0, 0.0, 6.0, 0.0, 0.01, self.object.now_color());
                }
                JudgeLineKind::Texture(texture) => {
                    let hw = texture.width() / 2.;
                    let hh = texture.height() / 2.;
                    draw_texture_ex(
                        *texture,
                        -hw,
                        -hh,
                        self.object.now_color(),
                        DrawTextureParams {
                            dest_size: Some(vec2(hw * 2., hh * 2.)),
                            flip_y: true,
                            ..Default::default()
                        },
                    )
                }
                JudgeLineKind::Text(anim) => {
                    let now = anim.now();
                    let size = 100;
                    let scale = 0.0008;
                    let dim = measure_text(&now, Some(res.font), size, scale);
                    Matrix::identity()
                        .append_nonuniform_scaling(&Vector::new(1.0, -1.0))
                        .apply_render(|| {
                            draw_text_ex(
                                &now,
                                -dim.width / 2.,
                                dim.offset_y - dim.height / 2.,
                                TextParams {
                                    font: res.font,
                                    font_size: size,
                                    font_scale: scale,
                                    color: self.object.now_color(),
                                    ..Default::default()
                                },
                            );
                        });
                }
            });
            let height = self.height.now();
            for note in &self.notes_above {
                note.render(res, height);
            }
            Matrix::identity()
                .append_nonuniform_scaling(&Vector::new(1.0, -1.0))
                .apply_render(|| {
                    for note in &self.notes_below {
                        note.render(res, height);
                    }
                });
        });
    }
}
