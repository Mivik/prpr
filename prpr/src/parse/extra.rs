use crate::{
    core::{Anim, BpmList, ChartExtra, Effect, Keyframe, Triple, Tweenable, Uniform, Video},
    ext::ScaleType,
    fs::FileSystem,
};
use anyhow::{anyhow, Context, Result};
use macroquad::prelude::{Color, Vec2};
use serde::Deserialize;
use std::{collections::HashMap, path::Path};

#[derive(Deserialize)]
struct ExtKeyframe<T> {
    time: Triple,
    value: T,
    #[serde(default)]
    easing: u8,
}

#[derive(Default, Deserialize)]
#[serde(untagged)]
enum ExtAnim<V> {
    #[default]
    Default,
    Fixed(V),
    Keyframes(Vec<ExtKeyframe<V>>),
}

impl<V> ExtAnim<V> {
    fn into<T: Tweenable>(self, r: &mut BpmList) -> Anim<T>
    where
        V: Into<T>,
    {
        match self {
            ExtAnim::Default => Anim::default(),
            ExtAnim::Fixed(value) => Anim::fixed(value.into()),
            ExtAnim::Keyframes(kfs) => Anim::new(
                kfs.into_iter()
                    .map(|it| Keyframe::new(r.time(&it.time), it.value.into(), it.easing))
                    .collect(),
            ),
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtBpmItem {
    time: Triple,
    bpm: f32,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BpmForm {
    Single(f32),
    List(Vec<ExtBpmItem>),
}

impl From<BpmForm> for BpmList {
    fn from(value: BpmForm) -> Self {
        match value {
            BpmForm::Single(value) => BpmList::new(vec![(0., value)]),
            BpmForm::List(list) => BpmList::new(list.into_iter().map(|it| (it.time.beats(), it.bpm)).collect()),
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Variable {
    Float(ExtAnim<f32>),
    Vec2(ExtAnim<(f32, f32)>),
    Color(ExtAnim<[u8; 4]>),
}

#[derive(Deserialize)]
struct ExtEffect {
    start: Triple,
    end: Triple,
    shader: String,
    #[serde(default)]
    vars: HashMap<String, Variable>,
    #[serde(default)]
    global: bool,
}

#[derive(Deserialize)]
struct ExtVideo {
    path: String,
    #[serde(default)]
    time: Triple,
    #[serde(default)]
    scale: ScaleType,
    #[serde(default)]
    alpha: ExtAnim<f32>,
    #[serde(default)]
    dim: ExtAnim<f32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Extra {
    bpm: BpmForm,
    #[serde(default)]
    effects: Vec<ExtEffect>,
    #[serde(default)]
    videos: Vec<ExtVideo>,
}

async fn parse_effect(r: &mut BpmList, rpe: ExtEffect, fs: &mut dyn FileSystem) -> Result<Effect> {
    let range = r.time(&rpe.start)..r.time(&rpe.end);
    let vars = rpe
        .vars
        .into_iter()
        .map(|(name, var)| -> Result<Box<dyn Uniform>> {
            Ok(match var {
                Variable::Float(events) => Box::new((name, events.into::<f32>(r))),
                Variable::Vec2(events) => Box::new((name, events.into::<Vec2>(r))),
                Variable::Color(events) => Box::new((name, events.into::<Color>(r))),
            })
        })
        .collect::<Result<_>>()?;
    let string;
    Effect::new(
        range,
        if let Some(path) = rpe.shader.strip_prefix('/') {
            string = String::from_utf8(fs.load_file(path).await?).with_context(|| format!("Cannot load shader from {path}"))?;
            &string
        } else {
            Effect::get_preset(&rpe.shader).ok_or_else(|| anyhow!("Cannot find preset shader {}", rpe.shader))?
        },
        vars,
        rpe.global,
    )
}

pub async fn parse_extra(source: &str, fs: &mut dyn FileSystem, ffmpeg: Option<&Path>) -> Result<ChartExtra> {
    let ext: Extra = serde_json::from_str(source).context("Failed to parse JSON")?;
    let mut r: BpmList = ext.bpm.into();
    let mut effects = Vec::new();
    let mut global_effects = Vec::new();
    for (id, effect) in ext.effects.into_iter().enumerate() {
        (if effect.global { &mut global_effects } else { &mut effects })
            .push(parse_effect(&mut r, effect, fs).await.with_context(|| format!("In effect #{id}"))?);
    }
    let mut videos = Vec::new();
    if let Some(ffmpeg) = ffmpeg {
        for video in ext.videos {
            videos.push(
                Video::new(
                    ffmpeg,
                    fs.load_file(&video.path)
                        .await
                        .with_context(|| format!("Failed to read video from {}", video.path))?,
                    r.time(&video.time),
                    video.scale,
                    video.alpha.into(&mut r),
                    video.dim.into(&mut r),
                )
                .with_context(|| format!("Failed to load video from {}", video.path))?,
            );
        }
    }
    Ok(ChartExtra {
        effects,
        global_effects,
        videos,
    })
}
