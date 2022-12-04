use super::{
    object::world_to_screen, Matrix, Object, Point, Resource, ScopedTransform, ASPECT_RATIO,
    NOTE_WIDTH_RATIO,
};
use macroquad::prelude::*;
use nalgebra::Translation2;

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
    pub multiple_hint: bool,
    pub fake: bool,
    pub last_real_time: f32,
}

fn draw_tex(
    res: &Resource,
    texture: Texture2D,
    x: f32,
    y: f32,
    color: Color,
    params: DrawTextureParams,
) {
    // TODO better rejection based on rectangle intersection
    let Vec2 { x: w, y: h } = params.dest_size.unwrap_or_else(|| {
        params
            .source
            .map(|it| vec2(it.w, it.h))
            .unwrap_or_else(|| vec2(texture.width(), texture.height()))
    });
    let pt1 = world_to_screen(res, Point::new(x, y));
    let pt2 = world_to_screen(res, Point::new(x + w, y));
    let pt3 = world_to_screen(res, Point::new(x, y + h));
    let pt4 = world_to_screen(res, Point::new(x + w, y + h));
    if pt1.x.min(pt2.x.min(pt3.x.min(pt4.x))) > 1.
        || pt1.x.max(pt2.x.max(pt3.x.max(pt4.x))) < -1.
        || pt1.y.min(pt2.y.min(pt3.y.min(pt4.y))) > 1.
        || pt1.y.max(pt2.y.max(pt3.y.max(pt4.y))) < -1.
    {
        return;
    }
    draw_texture_ex(texture, x, y, color, params);
}

impl Note {
    pub fn update(&mut self, res: &mut Resource, object: &mut Object) {
        if !self.fake
            && self.last_real_time < self.time
            && self.time <= res.real_time
            && (self.last_real_time - res.real_time).abs() < 0.5
        {
            res.audio_manager
                .play(match self.kind {
                    NoteKind::Click | NoteKind::Hold { .. } => res.sfx_click.clone(),
                    NoteKind::Drag => res.sfx_drag.clone(),
                    NoteKind::Flick => res.sfx_flick.clone(),
                })
                .unwrap();
            self.object.set_time(self.time);
            object.set_time(self.time);
            (object.now() * self.now_transform(0.)).apply_render(|| res.emit_at_origin());
        }
        self.last_real_time = res.real_time;
        self.object.set_time(res.time);
    }

    fn now_transform(&self, base: f32) -> Matrix {
        Translation2::new(0., base).to_homogeneous() * self.object.now() * self.object.now_scale()
    }

    pub fn render(&self, res: &mut Resource, line_height: f32) {
        let scale = (if self.multiple_hint { 1.1 } else { 1.0 }) * NOTE_WIDTH_RATIO;
        let color = self.object.now_color();

        let line_height = line_height / ASPECT_RATIO * self.speed;
        let height = self.height / ASPECT_RATIO * self.speed;

        let base = height - line_height;
        self.now_transform(base).apply_render(|| {
            let style = if self.multiple_hint {
                &res.note_style_mh
            } else {
                &res.note_style
            };
            let draw = |tex: Texture2D| {
                if res.time >= self.time {
                    return;
                }
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
            };
            match self.kind {
                NoteKind::Click => {
                    draw(style.click);
                }
                NoteKind::Hold {
                    end_time,
                    end_height,
                } => {
                    if res.time >= end_time {
                        return;
                    }
                    let end_height = end_height / ASPECT_RATIO * self.speed;
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
                    let en =
                        (tex.height() * ((end_height - h) / (end_height - height)).min(1.)).abs();
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
                                w: tex.width(),
                                h: en,
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
