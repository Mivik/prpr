use super::{
    object::world_to_screen, Object, Point, Resource, ScopedTransform, ASPECT_RATIO,
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
    pub fn set_time(&mut self, time: f32) {
        self.object.set_time(time);
    }

    pub fn render(&self, res: &Resource, line_height: f32) {
        let color = self.object.now_color();

        let line_height = line_height / ASPECT_RATIO * self.speed;
        let height = self.height / ASPECT_RATIO * self.speed;

        let style = if self.multiple_hint {
            &res.note_style_mh
        } else {
            &res.note_style
        };

        let draw = |tex: Texture2D| {
            if res.time >= self.time {
                return;
            }
            let h = height - line_height;
            let hf = vec2(
                NOTE_WIDTH_RATIO,
                tex.height() * NOTE_WIDTH_RATIO / tex.width(),
            );
            (Translation2::new(0., h).to_homogeneous() * self.object.now_scale()).apply_render(
                || {
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
                    )
                },
            );
        };
        let tr = self.object.now();
        /*tr.apply_render(|| {
            let s = format!("#{id}");
            let rect = measure_text(&s, None, 100, 0.001);
            draw_text_ex(
                &s,
                -rect.width / 2.,
                height - line_height - rect.height / 2.,
                TextParams {
                    font_size: 100,
                    font_scale: 0.001,
                    color: WHITE,
                    ..Default::default()
                },
            );
        });*/
        tr.apply_render(|| match self.kind {
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
                (Translation2::new(0.0, base).to_homogeneous() * self.object.now_scale())
                    .apply_render(|| {
                        // head
                        if res.time < self.time {
                            let tex = style.hold_head;
                            let hf = vec2(
                                NOTE_WIDTH_RATIO,
                                tex.height() * NOTE_WIDTH_RATIO / tex.width(),
                            );
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
                        let w = NOTE_WIDTH_RATIO;
                        // let h = line_height.max(height);
                        let h = if self.time <= res.time {
                            line_height
                        } else {
                            height
                        };
                        // TODO (end_height - height) is not always total height
                        let en = (tex.height()
                            * ((end_height - h) / (end_height - height)).min(1.))
                        .abs();
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
                            tex.height() * NOTE_WIDTH_RATIO / tex.width(),
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
                    });
            }
            NoteKind::Flick => {
                draw(style.flick);
            }
            NoteKind::Drag => {
                draw(style.drag);
            }
        });
    }
}
