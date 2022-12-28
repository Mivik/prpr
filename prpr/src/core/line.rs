use super::{Anim, AnimFloat, Matrix, Note, Object, Point, RenderConfig, Resource, Vector, EPS};
use crate::{
    ext::{draw_text_aligned, NotNanExt},
    judge::JudgeStatus,
};
use macroquad::prelude::*;

#[derive(Default)]
pub enum JudgeLineKind {
    #[default]
    Normal,
    Texture(Texture2D),
    Text(Anim<String>),
}

pub struct JudgeLineCache {
    update_order: Vec<u32>,
    start_index_above: usize,
    start_index_below: usize,
}

impl JudgeLineCache {
    pub fn new(notes: &mut Vec<Note>) -> Self {
        notes.sort_by_key(|it| (it.plain(), !it.above, (it.height + it.object.translation.1.now()).not_nan(), it.kind.order()));
        let mut res = Self {
            update_order: Vec::new(),
            start_index_above: 0,
            start_index_below: 0,
        };
        res.reset(notes);
        res
    }

    pub(crate) fn reset(&mut self, notes: &mut Vec<Note>) {
        self.update_order = (0..notes.len() as u32).collect();
        self.start_index_above = notes.iter().position(|it| it.plain()).unwrap_or(notes.len());
        self.start_index_below = notes[self.start_index_above..]
            .iter()
            .position(|it| !it.above)
            .map(|it| it + self.start_index_above)
            .unwrap_or(notes.len());
    }
}

pub struct JudgeLine {
    pub object: Object,
    pub kind: JudgeLineKind,
    pub height: AnimFloat,
    pub notes: Vec<Note>,
    pub color: Anim<Color>,
    pub parent: Option<usize>,
    pub z_index: i32,
    pub show_below: bool,

    pub cache: JudgeLineCache,
}

impl JudgeLine {
    pub fn update(&mut self, res: &mut Resource) {
        self.cache.update_order.retain(|id| {
            let note = &mut self.notes[*id as usize];
            note.update(res, &mut self.object);
            !note.dead()
        });
        self.object.set_time(res.time);
        if let JudgeLineKind::Text(anim) = &mut self.kind {
            anim.set_time(res.time);
        }
        self.color.set_time(res.time);
        self.height.set_time(res.time);
        while matches!(self.notes.get(self.cache.start_index_above).filter(|it| it.above).map(|note| &note.judge), Some(JudgeStatus::Judged)) {
            self.cache.start_index_above += 1;
        }
        while matches!(self.notes.get(self.cache.start_index_below).map(|note| &note.judge), Some(JudgeStatus::Judged)) {
            self.cache.start_index_below += 1;
        }
    }

    pub fn render(&self, res: &mut Resource, lines: &[JudgeLine]) {
        let alpha = self.object.alpha.now_opt().unwrap_or(1.0) * res.alpha;
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
                            let len = res.info.line_length;
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
                            res.apply_model_of(&Matrix::identity().append_nonuniform_scaling(&Vector::new(1., -1.)), || {
                                draw_text_aligned(res.font, &now, 0., 0., (0.5, 0.5), 1., color);
                            });
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
                let (vw, vh) = (1.1, 1.);
                let p = [
                    res.screen_to_world(Point::new(-vw, -vh)),
                    res.screen_to_world(Point::new(-vw, vh)),
                    res.screen_to_world(Point::new(vw, -vh)),
                    res.screen_to_world(Point::new(vw, vh)),
                ];
                let height_above = p[0].y.max(p[1].y.max(p[2].y.max(p[3].y))) * res.aspect_ratio;
                let height_below = -p[0].y.min(p[1].y.min(p[2].y.min(p[3].y))) * res.aspect_ratio;
                let agg = res.config.aggressive;
                for note in self.notes.iter().take_while(|it| !it.plain()).filter(|it| it.above) {
                    note.render(res, height, &config);
                }
                for note in self.notes[self.cache.start_index_above..].iter() {
                    if !note.above {
                        break;
                    }
                    if agg && note.height - height + note.object.translation.1.now() > height_above {
                        break;
                    }
                    note.render(res, height, &config);
                }
                res.with_model(Matrix::identity().append_nonuniform_scaling(&Vector::new(1.0, -1.0)), |res| {
                    for note in self.notes.iter().take_while(|it| !it.plain()).filter(|it| !it.above) {
                        note.render(res, height, &config);
                    }
                    for note in self.notes[self.cache.start_index_below..].iter() {
                        if agg && note.height - height + note.object.translation.1.now() > height_below {
                            break;
                        }
                        note.render(res, height, &config);
                    }
                });
            },
        );
    }
}
