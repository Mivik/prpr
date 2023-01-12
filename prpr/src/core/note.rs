use super::{Matrix, Object, Point, Resource, Vector, JUDGE_LINE_GOOD_COLOR, JUDGE_LINE_PERFECT_COLOR};
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
            Self::Hold { .. } => 0,
            Self::Drag => 1,
            Self::Click => 2,
            Self::Flick => 3,
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

fn draw_tex(res: &Resource, texture: Texture2D, order: i8, x: f32, y: f32, color: Color, params: DrawTextureParams) {
    let Vec2 { x: w, y: h } = params.dest_size.unwrap_or_else(|| {
        params
            .source
            .map(|it| vec2(it.w, it.h))
            .unwrap_or_else(|| vec2(texture.width(), texture.height()))
    });
    let p = [
        res.world_to_screen(Point::new(x, y)),
        res.world_to_screen(Point::new(x + w, y)),
        res.world_to_screen(Point::new(x + w, y + h)),
        res.world_to_screen(Point::new(x, y + h)),
    ];
    draw_tex_pts(res, texture, order, p, color, params);
}
fn draw_tex_pts(res: &Resource, texture: Texture2D, order: i8, mut p: [Point; 4], color: Color, params: DrawTextureParams) {
    if p[0].x.min(p[1].x.min(p[2].x.min(p[3].x))) > 1.
        || p[0].x.max(p[1].x.max(p[2].x.max(p[3].x))) < -1.
        || p[0].y.min(p[1].y.min(p[2].y.min(p[3].y))) > 1.
        || p[0].y.max(p[1].y.max(p[2].y.max(p[3].y))) < -1.
    {
        return;
    }
    let Rect { x: sx, y: sy, w: sw, h: sh } = params.source.unwrap_or(Rect { x: 0., y: 0., w: 1., h: 1. });

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
    res.note_buffer
        .borrow_mut()
        .push((order, texture.raw_miniquad_texture_handle().gl_internal_id()), vertices);
}

fn draw_center(res: &Resource, tex: Texture2D, order: i8, scale: f32, color: Color) {
    let hf = vec2(scale, tex.height() * scale / tex.width());
    draw_tex(
        res,
        tex,
        order,
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
        !self.fake && !matches!(self.kind, NoteKind::Hold { .. }) && self.speed == 1.0 && self.object.translation.1.keyframes.len() <= 1
    }

    pub fn update(&mut self, res: &mut Resource, parent_tr: &Matrix) {
        self.object.set_time(res.time);
        if let Some(color) = if let JudgeStatus::Hold(perfect, at, ..) = &mut self.judge {
            if res.time > *at {
                *at += HOLD_PARTICLE_INTERVAL / res.config.speed;
                Some(if *perfect { JUDGE_LINE_PERFECT_COLOR } else { JUDGE_LINE_GOOD_COLOR })
            } else {
                None
            }
        } else {
            None
        } {
            res.with_model(parent_tr * self.now_transform(res, 0.), |res| res.emit_at_origin(color));
        }
    }

    pub fn dead(&self) -> bool {
        (!matches!(self.kind, NoteKind::Hold { .. }) || matches!(self.judge, JudgeStatus::Judged)) && self.object.dead()
    }

    pub fn now_transform(&self, res: &Resource, base: f32) -> Matrix {
        self.object.now(res).append_translation(&Vector::new(0., base)) * self.object.now_scale()
    }

    pub fn render(&self, res: &mut Resource, line_height: f32, config: &RenderConfig) {
        if self.time - config.appear_before > res.time || (matches!(self.judge, JudgeStatus::Judged) && !matches!(self.kind, NoteKind::Hold { .. })) {
            return;
        }
        let scale = (if self.multiple_hint {
            res.skin.note_style_mh.click.width() / res.skin.note_style.click.width()
        } else {
            1.0
        }) * res.note_width;
        let mut color = self.object.now_color();
        color.a *= res.alpha;

        let line_height = line_height / res.aspect_ratio * self.speed;
        let height = self.height / res.aspect_ratio * self.speed;

        let base = height - line_height;
        if !config.draw_below
            && ((res.time - FADEOUT_TIME >= self.time) || (self.fake && res.time >= self.time) || (self.time > res.time && base <= -1e-5))
            && !matches!(self.kind, NoteKind::Hold { .. })
        {
            return;
        }
        let order = self.kind.order();
        res.with_model(self.now_transform(res, base), |res| {
            let style = if res.config.multiple_hint && self.multiple_hint {
                &res.skin.note_style_mh
            } else {
                &res.skin.note_style
            };
            let draw = |tex: Texture2D| {
                let mut color = color;
                if !config.draw_below {
                    color.a *= (self.time - res.time).min(0.) / FADEOUT_TIME + 1.;
                }
                draw_center(res, tex, order, scale, color);
            };
            match self.kind {
                NoteKind::Click => {
                    draw(*style.click);
                }
                NoteKind::Hold { end_time, end_height } => {
                    if matches!(self.judge, JudgeStatus::Judged) {
                        // miss
                        color.a *= 0.5;
                    }
                    if res.time >= end_time {
                        return;
                    }
                    let end_height = end_height / res.aspect_ratio * self.speed;
                    let base = height - line_height;
                    let pt = |x: f32, y: f32| res.world_to_screen(Point::new(x, y));

                    let h = if self.time <= res.time { line_height } else { height };
                    let th = h - line_height - base;
                    let btn = [pt(-scale, th), pt(scale, th)];
                    let th = end_height - line_height - base;
                    let top = [pt(-scale, th), pt(scale, th)];
                    let tex = &style.hold;
                    let ratio = style.hold_ratio();
                    // head
                    if res.time < self.time {
                        let r = style.hold_head_rect();
                        let hf = vec2(scale, r.h / r.w * scale * ratio);
                        draw_tex_pts(
                            res,
                            **tex,
                            order,
                            [btn[0], btn[1], pt(hf.x, -hf.y * 2.), pt(-hf.x, -hf.y * 2.)],
                            color,
                            DrawTextureParams {
                                source: Some(r),
                                ..Default::default()
                            },
                        );
                    }
                    // body
                    // TODO (end_height - height) is not always total height
                    draw_tex_pts(
                        res,
                        **tex,
                        order,
                        [top[0], top[1], btn[1], btn[0]],
                        color,
                        DrawTextureParams {
                            source: Some(style.hold_body_rect()),
                            ..Default::default()
                        },
                    );
                    // tail
                    let r = style.hold_tail_rect();
                    let hf = vec2(scale, r.h / r.w * scale * ratio);
                    let th = th + hf.y * 2.;
                    draw_tex_pts(
                        res,
                        **tex,
                        order,
                        [pt(-scale, th), pt(scale, th), top[1], top[0]],
                        color,
                        DrawTextureParams {
                            source: Some(r),
                            ..Default::default()
                        },
                    );
                }
                NoteKind::Flick => {
                    draw(*style.flick);
                }
                NoteKind::Drag => {
                    draw(*style.drag);
                }
            }
        });
    }
}

pub struct BadNote {
    pub time: f32,
    pub kind: NoteKind,
    pub matrix: Matrix,
}

impl BadNote {
    pub fn render(&self, res: &mut Resource) -> bool {
        if res.time > self.time + BAD_TIME {
            return false;
        }
        res.with_model(self.matrix, |res| {
            let style = &res.skin.note_style;
            draw_center(
                res,
                match &self.kind {
                    NoteKind::Click => *style.click,
                    NoteKind::Drag => *style.drag,
                    NoteKind::Flick => *style.flick,
                    _ => unreachable!(),
                },
                self.kind.order(),
                res.note_width,
                Color::new(0.423529, 0.262745, 0.262745, (self.time - res.time).max(-1.) / BAD_TIME + 1.),
            );
        });
        true
    }
}
