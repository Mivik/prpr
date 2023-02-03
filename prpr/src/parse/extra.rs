use crate::{
    core::{Anim, BpmList, ChartExtra, Effect, Keyframe, Triple, Tweenable, Uniform},
    fs::FileSystem,
};
use anyhow::{anyhow, Context, Result};
use macroquad::prelude::{Color, Vec2};
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExtBpmItem {
    time: Triple,
    bpm: f32,
}

#[derive(Deserialize)]
struct ExtKeyframe<T = f32> {
    time: Triple,
    value: T,
    #[serde(default)]
    easing: u8,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum Variable {
    Float(Vec<ExtKeyframe<f32>>),
    Vec2(Vec<ExtKeyframe<(f32, f32)>>),
    Color(Vec<ExtKeyframe<[u8; 4]>>),
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
#[serde(rename_all = "camelCase")]
struct Extra {
    bpm: BpmForm,
    #[serde(default)]
    effects: Vec<ExtEffect>,
}

#[inline]
fn parse_events<T: Tweenable, V: Clone + Into<T>>(r: &mut BpmList, kfs: &[ExtKeyframe<V>]) -> Anim<T> {
    Anim::new(
        kfs.iter()
            .map(|it| Keyframe::new(r.time(&it.time), it.value.clone().into(), it.easing))
            .collect(),
    )
}

async fn parse_effect(r: &mut BpmList, rpe: ExtEffect, fs: &mut dyn FileSystem) -> Result<Effect> {
    let range = r.time(&rpe.start)..r.time(&rpe.end);
    let vars = rpe
        .vars
        .into_iter()
        .map(|(name, var)| -> Result<Box<dyn Uniform>> {
            Ok(match var {
                Variable::Float(events) => Box::new((name, parse_events::<f32, f32>(r, &events))),
                Variable::Vec2(events) => Box::new((name, parse_events::<Vec2, (f32, f32)>(r, &events))),
                Variable::Color(events) => Box::new((name, parse_events::<Color, [u8; 4]>(r, &events))),
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

pub async fn parse_extra(source: &str, fs: &mut dyn FileSystem) -> Result<ChartExtra> {
    let ext: Extra = serde_json::from_str(source).context("Failed to parse JSON")?;
    let mut r: BpmList = ext.bpm.into();
    let mut effects = Vec::new();
    let mut global_effects = Vec::new();
    for (id, effect) in ext.effects.into_iter().enumerate() {
        (if effect.global { &mut global_effects } else { &mut effects })
            .push(parse_effect(&mut r, effect, fs).await.with_context(|| format!("In effect #{id}"))?);
    }
    Ok(ChartExtra { effects, global_effects })
}
