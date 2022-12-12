use super::{
    Matrix, Object, Point, Resource, Vector, JUDGE_LINE_GOOD_COLOR, JUDGE_LINE_PERFECT_COLOR,
    NOTE_WIDTH_RATIO,
};
use crate::judge::JudgeStatus;
use macroquad::prelude::*;

const HOLD_PARTICLE_INTERVAL: f32 = 0.15;
const FADEOUT_TIME: f32 = 0.16;
const BAD_TIME: f32 = 0.5;

#[derive(Clone, Debug)]
pub enum NoteKind {
    Click,
    Hold { end_time: f32, end_height: f32 },
    Flick,
    Drag,
}

impl NoteKind {
    pub fn order(&self) -> i8 {
        match self {
            Self::Click => 0,
            Self::Hold { .. } => 1,
            Self::Flick => 2,
            Self::Drag => 3,
        }
    }
}

pub struct Note {
    pub object: Object,
    pub kind: NoteKind,
    pub time: f32,
    pub height: f32,
    pub speed: f32,

    pub above: bool,
    pub multiple_hint: bool,
    pub fake: bool,
    pub judge: JudgeStatus,
}

pub struct RenderConfig {
    pub appear_before: f32,
    pub draw_below: bool,
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self {
            appear_before: f32::INFINITY,
            draw_below: true,
        }
    }
}

fn draw_tex(
    res: &Resource,
    texture: Texture2D,
    x: f32,
    y: f32,
    color: Color,
    params: DrawTextureParams,
) {
    let Vec2 { x: w, y: h } = params.dest_size.unwrap_or_else(|| {
        params
            .source
            .map(|it| vec2(it.w, it.h))
            .unwrap_or_else(|| vec2(texture.width(), texture.height()))
    });
    let mut p = [
        res.world_to_screen(Point::new(x, y)),
        res.world_to_screen(Point::new(x + w, y)),
        res.world_to_screen(Point::new(x + w, y + h)),
        res.world_to_screen(Point::new(x, y + h)),
    ];
    if p[0].x.min(p[1].x.min(p[2].x.min(p[3].x))) > 1.
        || p[0].x.max(p[1].x.max(p[2].x.max(p[3].x))) < -1.
        || p[0].y.min(p[1].y.min(p[2].y.min(p[3].y))) > 1.
        || p[0].y.max(p[1].y.max(p[2].y.max(p[3].y))) < -1.
    {
        return;
    }
    let gl = unsafe { get_internal_gl() }.quad_gl;

    let Rect {
        x: sx,
        y: sy,
        w: sw,
        h: sh,
    } = params.source.unwrap_or(Rect {
        x: 0.,
        y: 0.,
        w: 1.,
        h: 1.,
    });

    if params.flip_x {
        p.swap(0, 1);
        p.swap(2, 3);
    }
    if params.flip_y {
        p.swap(0, 3);
        p.swap(1, 2);
    }

    #[rustfmt::skip]
    let vertices = [
        Vertex::new(p[0].x, p[0].y, 0., sx     , sy     , color),
        Vertex::new(p[1].x, p[1].y, 0., sx + sw, sy     , color),
        Vertex::new(p[2].x, p[2].y, 0., sx + sw, sy + sh, color),
        Vertex::new(p[3].x, p[3].y, 0., sx     , sy + sh, color),
    ];
    let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

    gl.texture(Some(texture));
    gl.draw_mode(DrawMode::Triangles);
    gl.geometry(&vertices, &indices);
}

fn draw_center(res: &Resource, tex: Texture2D, scale: f32, color: Color) {
    let hf = vec2(scale, tex.height() * scale / tex.width());
    draw_tex(
        res,
        tex,
        -hf.x,
        -hf.y,
        color,
        DrawTextureParams {
            dest_size: Some(hf * 2.),
            flip_y: true,
            ..Default::default()
        },
    );
}

impl Note {
    pub fn plain(&self) -> bool {
        !self.fake
            && !matches!(self.kind, NoteKind::Hold { .. })
            && self.speed == 1.0
            && self.object.translation.1.keyframes.len() <= 1
    }

    pub fn update(&mut self, res: &mut Resource, object: &mut Object) {
        if let Some(color) = if let JudgeStatus::Hold(perfect, at, _) = &mut self.judge {
            if res.time > *at {
                *at += HOLD_PARTICLE_INTERVAL;
                Some(if *perfect {
                    JUDGE_LINE_PERFECT_COLOR
                } else {
                    JUDGE_LINE_GOOD_COLOR
                })
            } else {
                None
            }
        } else {
            None
        } {
            self.object.set_time(res.time);
            object.set_time(res.time);
            res.with_model(object.now(res) * self.now_transform(res, 0.), |res| {
                res.emit_at_origin(color)
            });
        }
        self.object.set_time(res.time);
    }

    pub fn dead(&self) -> bool {
        (!matches!(self.kind, NoteKind::Hold { .. }) || matches!(self.judge, JudgeStatus::Judged))
            && self.object.dead()
    }

    pub fn now_transform(&self, res: &Resource, base: f32) -> Matrix {
        self.object
            .now(res)
            .append_translation(&Vector::new(0., base))
            * self.object.now_scale()
    }

    pub fn render(&self, res: &mut Resource, line_height: f32, config: &RenderConfig) {
        if self.time - config.appear_before > res.time
            || (matches!(self.judge, JudgeStatus::Judged)
                && !matches!(self.kind, NoteKind::Hold { .. }))
        {
            return;
        }
        let scale = (if self.multiple_hint { 1.1 } else { 1.0 }) * NOTE_WIDTH_RATIO;
        let mut color = WHITE;

        let line_height = line_height / res.config.aspect_ratio * self.speed;
        let height = self.height / res.config.aspect_ratio * self.speed;

        let base = height - line_height;
        if !config.draw_below
            && (res.time - FADEOUT_TIME >= self.time || (self.fake && res.time >= self.time))
            && !matches!(self.kind, NoteKind::Hold { .. })
        {
            return;
        }
        res.with_model(self.now_transform(res, base), |res| {
            let style = if res.config.multiple_hint && self.multiple_hint {
                &res.note_style_mh
            } else {
                &res.note_style
            };
            let draw = |tex: Texture2D| {
                let mut color = color;
                if !config.draw_below {
                    color.a *= (self.time - res.time).min(0.) / FADEOUT_TIME + 1.;
                }
                draw_center(res, tex, scale, color);
            };
            match self.kind {
                NoteKind::Click => {
                    draw(style.click);
                }
                NoteKind::Hold {
                    end_time,
                    end_height,
                } => {
                    if matches!(self.judge, JudgeStatus::Judged) {
                        // miss
                        color.a *= 0.5;
                    }
                    if res.time >= end_time {
                        return;
                    }
                    let end_height = end_height / res.config.aspect_ratio * self.speed;
                    let base = height - line_height;
                    // head
                    if res.time < self.time {
                        let tex = style.hold_head;
                        let hf = vec2(scale, tex.height() * scale / tex.width());
                        draw_tex(
                            res,
                            tex,
                            -hf.x,
                            -hf.y * 2.,
                            color,
                            DrawTextureParams {
                                dest_size: Some(hf * 2.),
                                flip_y: true,
                                ..Default::default()
                            },
                        );
                    }
                    // body
                    let tex = style.hold;
                    let w = scale;
                    let h = if self.time <= res.time {
                        line_height
                    } else {
                        height
                    };
                    // TODO (end_height - height) is not always total height
                    draw_tex(
                        res,
                        tex,
                        -w,
                        h - line_height - base,
                        color,
                        DrawTextureParams {
                            source: Some(Rect {
                                x: 0.,
                                y: 0.,
                                w: 1.,
                                h: ((end_height - h) / (end_height - height)).min(1.).abs(),
                            }),
                            dest_size: Some(vec2(w * 2., end_height - h)),
                            flip_y: true,
                            ..Default::default()
                        },
                    );
                    // tail
                    let tex = style.hold_tail;
                    let hf = vec2(
                        NOTE_WIDTH_RATIO,
                        tex.height() / tex.width() * NOTE_WIDTH_RATIO,
                    );
                    draw_tex(
                        res,
                        tex,
                        -hf.x,
                        end_height - line_height - base,
                        color,
                        DrawTextureParams {
                            dest_size: Some(hf * 2.),
                            flip_y: true,
                            ..Default::default()
                        },
                    );
                }
                NoteKind::Flick => {
                    draw(style.flick);
                }
                NoteKind::Drag => {
                    draw(style.drag);
                }
            }
        });
    }
}

pub struct BadNote {
    pub time: f32,
    pub kind: NoteKind,
    pub matrix: Matrix,
    pub speed: Vector,
}

impl BadNote {
    pub fn render(&self, res: &mut Resource) -> bool {
        if res.time > self.time + BAD_TIME {
            return false;
        }
        res.with_model(self.matrix, |res| {
            res.apply_model(|| {
                draw_center(
                    res,
                    match &self.kind {
                        NoteKind::Click => res.note_style.click,
                        NoteKind::Drag => res.note_style.drag,
                        NoteKind::Flick => res.note_style.flick,
                        _ => unreachable!(),
                    },
                    NOTE_WIDTH_RATIO,
                    Color::new(
                        0.423529,
                        0.262745,
                        0.262745,
                        (self.time - res.time).min(0.) / BAD_TIME + 1.,
                    ),
                )
            });
        });
        true
    }
}
