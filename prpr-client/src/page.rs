mod about;
pub use about::AboutPage;

mod account;
pub use account::AccountPage;

mod local;
pub use local::LocalPage;

mod message;
pub use message::MessagePage;

mod remote;
pub use remote::RemotePage;

mod settings;
pub use settings::SettingsPage;

use crate::{
    cloud::{Images, LCFile},
    data::BriefChartInfo,
    dir, get_data,
    scene::ChartOrder,
};
use anyhow::Result;
use image::DynamicImage;
use lyon::{
    math as lm,
    path::{builder::BorderRadii, Path, Winding},
};
use macroquad::prelude::*;
use prpr::{
    ext::{SafeTexture, BLACK_TEXTURE},
    fs,
    task::Task,
    ui::{Scroll, Ui},
};
use std::{borrow::Cow, ops::DerefMut, sync::atomic::AtomicBool};

const ROW_NUM: u32 = 4;
const CARD_HEIGHT: f32 = 0.3;
const CARD_PADDING: f32 = 0.02;
const SIDE_PADDING: f32 = 0.02;

pub static SHOULD_UPDATE: AtomicBool = AtomicBool::new(false);

pub fn illustration_task(path: String) -> Task<Result<(DynamicImage, Option<DynamicImage>)>> {
    Task::new(async move {
        let mut fs = fs::fs_from_file(std::path::Path::new(&format!("{}/{}", dir::charts()?, path)))?;
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

pub struct ChartItem {
    pub info: BriefChartInfo,
    pub path: String,
    pub illustration: (SafeTexture, SafeTexture),
    pub illustration_task: Option<Task<Result<(DynamicImage, Option<DynamicImage>)>>>,
}

pub struct SharedState {
    pub t: f32,
    pub content_size: (f32, f32),
    pub tex: SafeTexture,

    pub charts_local: Vec<ChartItem>,
    pub charts_remote: Vec<ChartItem>,

    pub transit: Option<(Option<LCFile>, u32, f32, Rect, bool)>, // remote, id, start_time, rect, delete
}

impl SharedState {
    pub async fn new() -> Result<Self> {
        let tex: SafeTexture = Texture2D::from_image(&load_image("player.jpg").await?).into();
        let charts_local = load_local(&tex, &(ChartOrder::Default, false));
        Ok(Self {
            t: 0.,
            content_size: (0., 0.),
            tex,

            charts_local,
            charts_remote: Vec::new(),

            transit: None,
        })
    }

    fn render_scroll(ui: &mut Ui, content_size: (f32, f32), scroll: &mut Scroll, charts: &mut Vec<ChartItem>) {
        scroll.size(content_size);
        let sy = scroll.y_scroller.offset();
        scroll.render(ui, |ui| {
            let cw = content_size.0 / ROW_NUM as f32;
            let ch = CARD_HEIGHT;
            let p = CARD_PADDING;
            let path = {
                let mut path = Path::builder();
                path.add_rounded_rectangle(&lm::Box2D::new(lm::point(p, p), lm::point(cw - p, ch - p)), &BorderRadii::new(0.01), Winding::Positive);
                path.build()
            };
            let start_line = (sy / ch) as u32;
            let end_line = ((sy + content_size.1) / ch).ceil() as u32;
            let range = (start_line * ROW_NUM)..((end_line + 1) * ROW_NUM);
            ui.hgrids(content_size.0, ch, ROW_NUM, charts.len() as u32, |ui, id| {
                if !range.contains(&id) {
                    return;
                }
                let chart = &mut charts[id as usize];
                if let Some(task) = &mut chart.illustration_task {
                    if let Some(image) = task.take() {
                        chart.illustration = if let Ok(image) = image {
                            Images::into_texture(image)
                        } else {
                            (BLACK_TEXTURE.clone(), BLACK_TEXTURE.clone())
                        };
                        chart.illustration_task = None;
                    }
                }
                ui.fill_path(&path, (*chart.illustration.0, Rect::new(0., 0., cw, ch)));
                ui.fill_path(&path, Color::new(0., 0., 0., 0.55));
                ui.text(&chart.info.name)
                    .pos(p + 0.01, ch - p - 0.02)
                    .max_width(cw - p * 2.)
                    .anchor(0., 1.)
                    .size(0.6)
                    .draw();
            })
        });
    }
}

pub trait Page {
    fn label(&self) -> Cow<'static, str>;
    fn has_new(&self) -> bool {
        false
    }

    fn update(&mut self, focus: bool, state: &mut SharedState) -> Result<()>;
    fn touch(&mut self, touch: &Touch, state: &mut SharedState) -> Result<bool>;
    fn render(&mut self, ui: &mut Ui, state: &mut SharedState) -> Result<()>;
    fn pause(&mut self) -> Result<()> {
        Ok(())
    }
    fn resume(&mut self) -> Result<()> {
        Ok(())
    }
}
