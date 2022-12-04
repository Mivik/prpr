#![allow(dead_code)]
use super::{process_lines, BpmList, Triple, TWEEN_MAP};
use crate::{
    core::{
        Anim, AnimFloat, AnimVector, Chart, ClampedTween, JudgeLine, JudgeLineKind, Keyframe, Note,
        NoteKind, Object, StaticTween, HEIGHT_RATIO,
    },
    ext::NotNanExt,
};
use anyhow::{bail, Context, Result};
use macroquad::texture::{load_image, Texture2D};
use serde::Deserialize;
use std::rc::Rc;

const RPE_WIDTH: f32 = 1350.;
const RPE_HEIGHT: f32 = 900.;
const SPEED_RATIO: f32 = 10. / 45. / HEIGHT_RATIO;
const EPS: f32 = 1e-5;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEBpmItem {
    bpm: f32,
    start_time: Triple,
}

// serde is weird...
fn f32_zero() -> f32 {
    0.
}

fn f32_one() -> f32 {
    1.
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEEvent {
    // TODO linkgroup
    #[serde(default = "f32_zero")]
    easing_left: f32,
    #[serde(default = "f32_one")]
    easing_right: f32,
    easing_type: u8,
    start: f32,
    end: f32,
    start_time: Triple,
    end_time: Triple,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPESpeedEvent {
    // TODO linkgroup
    start_time: Triple,
    end_time: Triple,
    start: f32,
    end: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEEventLayer {
    alpha_events: Option<Vec<RPEEvent>>,
    move_x_events: Option<Vec<RPEEvent>>,
    move_y_events: Option<Vec<RPEEvent>>,
    rotate_events: Option<Vec<RPEEvent>>,
    speed_events: Option<Vec<RPESpeedEvent>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPETextEvent {
    #[serde(rename = "start")]
    text: String,
    start_time: Triple,
    end_time: Triple,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEExtendedEvents {
    text_events: Option<Vec<RPETextEvent>>,
    scale_x_events: Option<Vec<RPEEvent>>,
    scale_y_events: Option<Vec<RPEEvent>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPENote {
    // TODO above == 0? what does that even mean?
    #[serde(rename = "type")]
    kind: u8,
    above: u8,
    start_time: Triple,
    end_time: Triple,
    position_x: f32,
    y_offset: f32,
    alpha: u8,
    size: f32,
    speed: f32,
    is_fake: u8,
    visible_time: f32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEJudgeLine {
    // TODO group, parent, isCover
    // TODO alphaControl, bpmfactor
    #[serde(rename = "Name")]
    name: String,
    #[serde(rename = "Texture")]
    texture: String,
    event_layers: Vec<Option<RPEEventLayer>>,
    extended: Option<RPEExtendedEvents>,
    notes: Option<Vec<RPENote>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEMetadata {
    offset: i32,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RPEChart {
    #[serde(rename = "META")]
    meta: RPEMetadata,
    #[serde(rename = "BPMList")]
    bpm_list: Vec<RPEBpmItem>,
    judge_line_list: Vec<RPEJudgeLine>,
}

fn parse_events(r: &mut BpmList, rpe: &[RPEEvent]) -> Result<AnimFloat> {
    let mut kfs = Vec::new();
    for e in rpe {
        kfs.push(Keyframe {
            time: r.time(&e.start_time),
            value: e.start,
            tween: {
                let tween = TWEEN_MAP[e.easing_type as usize];
                if e.easing_left.abs() < EPS && (e.easing_right - 1.0).abs() < EPS {
                    StaticTween::get_rc(tween)
                } else {
                    Rc::new(ClampedTween::new(tween, e.easing_left..e.easing_right))
                }
            },
        });
        kfs.push(Keyframe::new(r.time(&e.end_time), e.end, 0));
    }
    Ok(AnimFloat::new(kfs))
}

fn parse_speed_events(r: &mut BpmList, rpe: &[RPEEventLayer], max_time: f32) -> Result<AnimFloat> {
    let rpe: Vec<_> = rpe
        .iter()
        .filter_map(|it| it.speed_events.as_ref())
        .collect();
    if rpe.is_empty() {
        // TODO or is it?
        return Ok(AnimFloat::default());
    };
    let anis: Vec<_> = rpe
        .into_iter()
        .map(|it| {
            let mut kfs = Vec::new();
            for e in it {
                kfs.push(Keyframe::new(r.time(&e.start_time), e.start, 2));
                kfs.push(Keyframe::new(r.time(&e.end_time), e.end, 0));
            }
            AnimFloat::new(kfs)
        })
        .collect();
    let mut pts: Vec<_> = anis
        .iter()
        .flat_map(|it| it.keyframes.iter().map(|it| it.time.not_nan()))
        .collect();
    pts.push(max_time.not_nan());
    pts.sort();
    pts.dedup();
    let mut sani = AnimFloat::chain(anis);
    sani.map_value(|v| v * SPEED_RATIO);
    let mut kfs = Vec::new();
    let mut height = 0.0;
    for i in 0..(pts.len() - 1) {
        let now_time = *pts[i];
        let end_time = *pts[i + 1];
        sani.set_time(now_time);
        let speed = sani.now();
        // this can affect a lot! do not use end_time...
        // using end_time causes Hold tween (x |-> 0) to be recognized as Linear tween (x |-> x)
        sani.set_time(end_time - 1e-4);
        let end_speed = sani.now();
        kfs.push(if (speed - end_speed).abs() < EPS {
            Keyframe::new(now_time, height, 2)
        } else if speed > end_speed {
            Keyframe {
                time: now_time,
                value: height,
                tween: Rc::new(ClampedTween::new(
                    7, /*quadOut*/
                    0.0..(1. - end_speed / speed),
                )),
            }
        } else {
            Keyframe {
                time: now_time,
                value: height,
                tween: Rc::new(ClampedTween::new(
                    6, /*quadIn*/
                    (speed / end_speed)..1.,
                )),
            }
        });
        height += (speed + end_speed) * (end_time - now_time) / 2.;
    }
    kfs.push(Keyframe::new(max_time, height, 0));
    Ok(AnimFloat::new(kfs))
}

fn parse_notes(
    r: &mut BpmList,
    rpe: Vec<RPENote>,
    height: &mut AnimFloat,
) -> Result<(Vec<Note>, Vec<Note>)> {
    let mut notes_above = Vec::new();
    let mut notes_below = Vec::new();
    for note in rpe {
        let dst = if note.above == 1 {
            &mut notes_above
        } else {
            &mut notes_below
        };
        // TODO visible_time, y_offset
        let time = r.time(&note.start_time);
        height.set_time(time);
        let note_height = height.now();
        dst.push(Note {
            object: Object {
                translation: AnimVector(
                    AnimFloat::fixed(note.position_x / (RPE_WIDTH / 2.)),
                    AnimFloat::default(),
                ),
                scale: AnimVector(
                    if note.size == 1.0 {
                        AnimFloat::default()
                    } else {
                        AnimFloat::fixed(note.size)
                    },
                    AnimFloat::default(),
                ),
                ..Default::default()
            },
            kind: match note.kind {
                1 => NoteKind::Click,
                2 => {
                    let end_time = r.time(&note.end_time);
                    height.set_time(end_time);
                    NoteKind::Hold {
                        end_time,
                        end_height: height.now(),
                    }
                }
                3 => NoteKind::Flick,
                4 => NoteKind::Drag,
                _ => bail!("Unknown note type: {}", note.kind),
            },
            time,
            height: note_height,
            speed: note.speed,
            multiple_hint: false,
            fake: note.is_fake != 0,
            last_real_time: 0.0,
        });
    }
    Ok((notes_above, notes_below))
}

fn parse_text_events(r: &mut BpmList, rpe: &[RPETextEvent]) -> Result<Anim<String>> {
    let mut kfs = Vec::new();
    if rpe[0].start_time.beats() != 0.0 {
        kfs.push(Keyframe::new(0.0, String::new(), 0));
    }
    for e in rpe {
        kfs.push(Keyframe::new(r.time(&e.start_time), e.text.clone(), 0));
    }
    Ok(Anim::new(kfs))
}

async fn parse_judge_line(r: &mut BpmList, rpe: RPEJudgeLine, max_time: f32) -> Result<JudgeLine> {
    let event_layers: Vec<_> = rpe.event_layers.into_iter().flatten().collect();
    fn events_with_factor(
        r: &mut BpmList,
        event_layers: &[RPEEventLayer],
        get: impl Fn(&RPEEventLayer) -> &Option<Vec<RPEEvent>>,
        factor: f32,
        desc: &str,
    ) -> Result<AnimFloat> {
        let anis: Vec<_> = event_layers
            .iter()
            .filter_map(|it| get(it).as_ref().map(|es| parse_events(r, es)))
            .collect::<Result<_>>()
            .with_context(|| format!("Failed to parse {desc} events"))?;
        let mut res = AnimFloat::chain(anis);
        res.map_value(|v| v * factor);
        Ok(res)
    }
    let mut height = parse_speed_events(r, &event_layers, max_time)?;
    let (notes_above, notes_below) = parse_notes(r, rpe.notes.unwrap_or_default(), &mut height)?;
    Ok(JudgeLine {
        object: Object {
            alpha: events_with_factor(r, &event_layers, |it| &it.alpha_events, 1. / 255., "alpha")?,
            rotation: events_with_factor(r, &event_layers, |it| &it.rotate_events, -1., "rotate")?,
            translation: AnimVector(
                events_with_factor(
                    r,
                    &event_layers,
                    |it| &it.move_x_events,
                    2. / RPE_WIDTH,
                    "move X",
                )?,
                events_with_factor(
                    r,
                    &event_layers,
                    |it| &it.move_y_events,
                    2. / RPE_HEIGHT,
                    "move Y",
                )?,
            ),
            scale: {
                fn parse(
                    r: &mut BpmList,
                    opt: &Option<Vec<RPEEvent>>,
                    factor: f32,
                ) -> Result<AnimFloat> {
                    let mut res = opt
                        .as_ref()
                        .map(|it| parse_events(r, it))
                        .transpose()?
                        .unwrap_or_default();
                    res.map_value(|v| v * factor);
                    Ok(res)
                }
                let factor = if rpe.texture == "line.png" {
                    1.
                } else {
                    1. / 1080.
                };
                rpe.extended
                    .as_ref()
                    .map(|e| -> Result<_> {
                        Ok(AnimVector(
                            parse(r, &e.scale_x_events, factor)?,
                            parse(r, &e.scale_y_events, factor)?,
                        ))
                    })
                    .transpose()?
                    .unwrap_or_default()
            },
            ..Default::default()
        },
        height,
        notes_above,
        notes_below,
        kind: if rpe.texture == "line.png" {
            if let Some(events) = &rpe.extended.and_then(|e| e.text_events) {
                JudgeLineKind::Text(
                    parse_text_events(r, events).context("Failed to parse text events")?,
                )
            } else {
                JudgeLineKind::Normal
            }
        } else {
            JudgeLineKind::Texture(Texture2D::from_image(
                &load_image(&format!("texture/{}", rpe.texture))
                    .await
                    .with_context(|| format!("Failed to load texture {}", rpe.texture))?,
            ))
        },
    })
}

pub async fn parse_rpe(source: &str) -> Result<Chart> {
    let rpe: RPEChart = serde_json::from_str(source).context("Failed to parse JSON")?;
    let mut r = BpmList::new(
        rpe.bpm_list
            .into_iter()
            .map(|it| (it.start_time.beats(), it.bpm))
            .collect(),
    );
    fn vec<T>(v: &Option<Vec<T>>) -> impl Iterator<Item = &T> {
        v.iter().flat_map(|it| it.iter())
    }
    #[rustfmt::skip]
    let max_time = *rpe
        .judge_line_list
        .iter()
        .map(|line| {
            line.notes.as_ref().map(|notes| {
                notes
                    .iter()
                    .map(|note| r.time(&note.start_time).not_nan())
                    .max()
                    .unwrap_or_default()
            }).unwrap_or_default().max(
                line.event_layers.iter().filter_map(|it| it.as_ref().map(|layer| {
                    vec(&layer.alpha_events)
                        .chain(vec(&layer.move_x_events))
                        .chain(vec(&layer.move_y_events))
                        .chain(vec(&layer.rotate_events))
                        .map(|it| r.time(&it.end_time).not_nan())
                        .max().unwrap_or_default()
                })).max().unwrap_or_default()
            ).max(
                line.extended.as_ref().map(|e| {
                    vec(&e.scale_x_events)
                        .chain(vec(&e.scale_y_events))
                        .map(|it| r.time(&it.end_time).not_nan())
                        .max().unwrap_or_default()
                        .max(vec(&e.text_events).map(|it| r.time(&it.end_time).not_nan()).max().unwrap_or_default())
                }).unwrap_or_default()
            )
        })
        .max().unwrap_or_default() + 1.;
    // don't want to add a whole crate for a mere join_all...
    let mut lines = Vec::new();
    for (id, rpe) in rpe.judge_line_list.into_iter().enumerate() {
        let name = rpe.name.clone();
        lines.push(
            parse_judge_line(&mut r, rpe, max_time)
                .await
                .with_context(move || format!("In judge line #{id} ({})", name))?,
        );
    }
    process_lines(&mut lines);
    Ok(Chart {
        offset: rpe.meta.offset as f32 / 1000.0,
        lines,
    })
}
