mod home;
pub use home::HomePage;

mod library;
pub use library::LibraryPage;

use crate::{
    data::BriefChartInfo,
    dir, get_data,
    images::Images,
    phizone::{PZChart, PZFile},
    scene::{fs_from_path, ChartOrder},
};
use anyhow::Result;
use image::DynamicImage;
use lyon::{
    math as lm,
    path::{builder::BorderRadii, Path, Winding},
};
use macroquad::prelude::*;
use prpr::{
    ext::{RectExt, SafeTexture, BLACK_TEXTURE},
    fs,
    scene::NextScene,
    task::Task,
    ui::{FontArc, Scroll, TextPainter, Ui},
};
use std::{
    borrow::Cow,
    ops::DerefMut,
    sync::{atomic::AtomicBool, Arc}, any::Any,
};

const ROW_NUM: u32 = 4;
const CARD_HEIGHT: f32 = 0.3;
const CARD_PADDING: f32 = 0.02;
const SIDE_PADDING: f32 = 0.02;

pub static SHOULD_UPDATE: AtomicBool = AtomicBool::new(false);

pub fn illustration_task(path: String) -> Task<Result<(DynamicImage, Option<DynamicImage>)>> {
    Task::new(async move {
        let mut fs = fs_from_path(&path)?;
        let info = fs::load_info(fs.deref_mut()).await?;
        let image = image::load_from_memory(&fs.load_file(&info.illustration).await?)?;
        let thumbnail =
            Images::local_or_else(format!("{}/{}", dir::cache_image_local()?, path.replace('/', "_")), async { Ok(Images::thumbnail(&image)) })
                .await?;
        Ok((thumbnail, Some(image)))
    })
}

fn get_touched(pos: (f32, f32)) -> Option<u32> {
    let row = (pos.1 / CARD_HEIGHT) as i32;
    if row < 0 {
        return None;
    }
    let width = (2. - SIDE_PADDING * 2.) / ROW_NUM as f32;
    let column = (pos.0 / width) as i32;
    if column < 0 || column >= ROW_NUM as i32 {
        return None;
    }
    let x = pos.0 - width * column as f32;
    if x < CARD_PADDING || x + CARD_PADDING >= width {
        return None;
    }
    let y = pos.1 - CARD_HEIGHT * row as f32;
    if y < CARD_PADDING || y + CARD_PADDING >= CARD_HEIGHT {
        return None;
    }
    let id = row as u32 * ROW_NUM + column as u32;
    Some(id)
}

fn trigger_grid(phase: TouchPhase, choose: &mut Option<u32>, id: Option<u32>) -> bool {
    match phase {
        TouchPhase::Started => {
            *choose = id;
            false
        }
        TouchPhase::Moved | TouchPhase::Stationary => {
            if *choose != id {
                *choose = None;
            }
            false
        }
        TouchPhase::Cancelled => {
            *choose = None;
            false
        }
        TouchPhase::Ended => choose.take() == id && id.is_some(),
    }
}

pub fn load_local(tex: &SafeTexture, order: &(ChartOrder, bool)) -> Vec<ChartItem> {
    let mut res: Vec<_> = get_data()
        .charts
        .iter()
        .map(|it| ChartItem {
            info: it.info.clone(),
            path: it.path.clone(),
            illustration: (tex.clone(), tex.clone()),
            illustration_task: Some(illustration_task(it.path.clone())),
        })
        .collect();
    order.0.apply(&mut res);
    if order.1 {
        res.reverse();
    }
    res
}

#[derive(Clone)]
pub struct ChartItem {
    pub info: BriefChartInfo,
    pub path: String,
    pub illustration: (SafeTexture, SafeTexture),
    pub illustration_task: Option<Task<Result<(DynamicImage, Option<DynamicImage>)>>>,
}

impl ChartItem {
    pub fn settle(&mut self) {
        if let Some(task) = &mut self.illustration_task {
            if let Some(illu) = task.take() {
                self.illustration = if let Ok(illu) = illu {
                    Images::into_texture(illu)
                } else {
                    (BLACK_TEXTURE.clone(), BLACK_TEXTURE.clone())
                };
                self.illustration_task = None;
            }
        }
    }
}

// srange name, isn't it?
pub struct Fader {
    distance: f32,
    start_time: f32,
    index: usize,
    back: bool,
    pub sub: bool,
}

impl Fader {
    const TIME: f32 = 0.7;
    const DELTA: f32 = 0.04;

    pub fn new() -> Self {
        Self {
            distance: 0.2,
            start_time: f32::NAN,
            index: 0,
            back: false,
            sub: false,
        }
    }

    #[inline]
    pub fn with_distance(mut self, distance: f32) -> Self {
        self.distance = distance;
        self
    }

    #[inline]
    pub fn reset(&mut self) {
        self.index = 0;
    }

    #[inline]
    pub fn sub(&mut self, t: f32) {
        self.start_time = t;
        self.back = false;
    }

    #[inline]
    pub fn for_sub<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        self.sub = true;
        let res = f(self);
        self.sub = false;
        res
    }

    #[inline]
    pub fn back(&mut self, t: f32) {
        self.start_time = t;
        self.back = true;
    }

    pub fn progress(&self, t: f32) -> f32 {
        if self.start_time.is_nan() {
            0.
        } else {
            let p = ((t - self.start_time) / Self::TIME).clamp(0., 1.);
            let p = (1. - p).powi(3);
            let p = if self.back { p } else { 1. - p };
            if self.sub {
                1. - p
            } else {
                -p
            }
        }
    }

    pub fn render<R>(&mut self, ui: &mut Ui, t: f32, f: impl FnOnce(&mut Ui, Color) -> R) -> R {
        let p = self.progress(t - self.index as f32 * Self::DELTA);
        let (dy, alpha) = (p * self.distance, 1. - p.abs());
        self.index += 1;
        ui.scope(|ui| {
            ui.dy(dy);
            f(ui, Color::new(1., 1., 1., alpha))
        })
    }

    #[inline]
    pub fn transiting(&self) -> bool {
        !self.start_time.is_nan()
    }

    pub fn done(&mut self, t: f32) -> Option<bool> {
        if !self.start_time.is_nan() && t - self.start_time > Self::TIME {
            self.start_time = f32::NAN;
            Some(self.back)
        } else {
            None
        }
    }

    pub fn render_title(&mut self, ui: &mut Ui, painter: &mut TextPainter, t: f32, s: &str) {
        let tp = -ui.top + 0.08;
        let h = ui.text("L").size(1.4).no_baseline().measure().h;
        ui.scissor(Some(Rect::new(-1., tp, 2., h)));
        let p = self.progress(t);
        let tp = tp + h * p;
        for (i, c) in s.chars().enumerate() {
            ui.text(c.to_string())
                .pos(-0.8 + i as f32 * 0.117, tp)
                .anchor(0.5, 0.)
                .size(1.4)
                .color(Color::new(1., 1., 1., 0.4))
                .draw_with_font(Some(painter));
        }
        ui.scissor(None);
    }
}

pub struct SharedState {
    pub t: f32,
    pub fader: Fader,
    pub painter: TextPainter,
    pub charts_local: Vec<ChartItem>,
}

impl SharedState {
    pub async fn new() -> Result<Self> {
        let font = FontArc::try_from_vec(load_file("halva.ttf").await?)?;
        let painter = TextPainter::new(font);
        Ok(Self {
            t: 0.,
            fader: Fader::new(),
            painter,
            charts_local: Vec::new(),
        })
    }

    pub fn render_fader<R>(&mut self, ui: &mut Ui, f: impl FnOnce(&mut Ui, Color) -> R) -> R {
        self.fader.render(ui, self.t, f)
    }
}

#[derive(Default)]
pub enum NextPage {
    #[default]
    None,
    Overlay(Box<dyn Page>),
    Pop,
}

pub trait Page {
    fn label(&self) -> Cow<'static, str>;

    fn on_result(&mut self, result: Box<dyn Any>, _s: &mut SharedState) -> Result<()> {
        Ok(())
    }
    fn enter(&mut self, _s: &mut SharedState) -> Result<()> {
        Ok(())
    }
    fn update(&mut self, s: &mut SharedState) -> Result<()>;
    fn touch(&mut self, touch: &Touch, s: &mut SharedState) -> Result<bool>;
    fn render(&mut self, ui: &mut Ui, s: &mut SharedState) -> Result<()>;
    fn pause(&mut self) -> Result<()> {
        Ok(())
    }
    fn resume(&mut self) -> Result<()> {
        Ok(())
    }
    fn next_page(&mut self) -> NextPage {
        NextPage::None
    }
    fn next_scene(&mut self) -> NextScene {
        NextScene::None
    }
}
