use super::{draw_text_aligned, Anim, AnimFloat, Matrix, Note, Object, Resource, Vector, EPS};
use crate::core::RenderConfig;
use macroquad::prelude::*;

#[derive(Default)]
pub enum JudgeLineKind {
    #[default]
    Normal,
    Texture(Texture2D),
    Text(Anim<String>),
}

pub struct JudgeLine {
    pub object: Object,
    pub kind: JudgeLineKind,
    pub height: AnimFloat,
    pub notes: Vec<Note>,
    pub color: Anim<Color>,
    pub parent: Option<usize>,
    pub show_below: bool,
}

impl JudgeLine {
    pub fn update(&mut self, res: &mut Resource) {
        for note in &mut self.notes {
            note.update(res, &mut self.object);
        }
        self.object.set_time(res.time);
        if let JudgeLineKind::Text(anim) = &mut self.kind {
            anim.set_time(res.time);
        }
        self.color.set_time(res.time);
        self.height.set_time(res.time);
    }

    pub fn render(&self, res: &mut Resource, lines: &[JudgeLine]) {
        let alpha = self.object.alpha.now_opt().unwrap_or(1.0);
        let color = self.color.now_opt();
        res.with_model(
            if let Some(parent) = self.parent {
                // TODO currently we're only resolving one layer
                lines[parent].object.now(res) * self.object.now(res)
            } else {
                self.object.now(res)
            },
            |res| {
                res.with_model(self.object.now_scale(), |res| {
                    res.apply_model(|| match &self.kind {
                        JudgeLineKind::Normal => {
                            let mut color = color.unwrap_or(res.judge_line_color);
                            color.a = alpha.max(0.0);
                            let len = res.config.line_length;
                            draw_line(-len, 0.0, len, 0.0, 0.01, color);
                        }
                        JudgeLineKind::Texture(texture) => {
                            let mut color = color.unwrap_or(WHITE);
                            color.a = alpha.max(0.0);
                            let hw = texture.width() / 2.;
                            let hh = texture.height() / 2.;
                            draw_texture_ex(
                                *texture,
                                -hw,
                                -hh,
                                color,
                                DrawTextureParams {
                                    dest_size: Some(vec2(hw * 2., hh * 2.)),
                                    flip_y: true,
                                    ..Default::default()
                                },
                            );
                        }
                        JudgeLineKind::Text(anim) => {
                            let mut color = color.unwrap_or(WHITE);
                            color.a = alpha.max(0.0);
                            let now = anim.now();
                            res.apply_model_of(
                                &Matrix::identity()
                                    .append_nonuniform_scaling(&Vector::new(1., -1.)),
                                || {
                                    draw_text_aligned(res, &now, 0., 0., (0.5, 0.5), 1., color);
                                },
                            );
                        }
                    })
                });
                let height = self.height.now();
                let mut config = RenderConfig {
                    draw_below: self.show_below,
                    ..Default::default()
                };
                if alpha < 0.0 {
                    let w = (-alpha.round()) as u32;
                    match w {
                        1 => {
                            return;
                        }
                        2 => {
                            config.draw_below = false;
                        }
                        w if (100..1000).contains(&w) => {
                            config.appear_before = (w as f32 - 100.) / 10.;
                        }
                        w if (1000..2000).contains(&w) => {
                            // TODO unsupported
                        }
                        _ => {}
                    }
                }
                if (alpha + 1.0).abs() < EPS {
                    return;
                }
                if -1000.0 < alpha && alpha <= -100.0 {}
                for note in self.notes.iter().filter(|it| it.above) {
                    note.render(res, height, &config);
                }
                res.with_model(
                    Matrix::identity().append_nonuniform_scaling(&Vector::new(1.0, -1.0)),
                    |res| {
                        for note in self.notes.iter().filter(|it| !it.above) {
                            note.render(res, height, &config);
                        }
                    },
                );
            },
        );
    }
}
