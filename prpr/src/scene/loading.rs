use super::{draw_background, draw_illustration, GameScene, NextScene, Scene};
use crate::{
    config::Config,
    ext::{draw_parallelogram, draw_text_aligned},
    fs::FileSystem,
    info::ChartInfo,
    time::TimeManager,
};
use anyhow::{Context, Result};
use image::ImageBuffer;
use macroquad::prelude::*;
use std::{
    future::Future,
    pin::Pin,
    task::{Poll, RawWaker, RawWakerVTable, Waker},
};

const BEFORE_TIME: f32 = 1.;
const TRANSITION_TIME: f32 = 1.4;
const WAIT_TIME: f32 = 0.4;

fn waker() -> Waker {
    unsafe fn clone(data: *const ()) -> RawWaker {
        RawWaker::new(data, &VTABLE)
    }
    unsafe fn wake(_data: *const ()) {
        panic!()
    }
    unsafe fn wake_by_ref(data: *const ()) {
        wake(data)
    }
    unsafe fn drop(_data: *const ()) {}
    const VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake_by_ref, drop);
    let raw_waker = RawWaker::new(std::ptr::null(), &VTABLE);
    unsafe { Waker::from_raw(raw_waker) }
}

pub struct LoadingScene {
    info: ChartInfo,
    illustration: Texture2D,
    background: Texture2D,
    font: Font,
    future: Option<Pin<Box<dyn Future<Output = Result<GameScene>>>>>,
    next_scene: Option<Box<dyn Scene>>,
    finish_time: f32,
    target: Option<RenderTarget>,
}

impl LoadingScene {
    pub const TOTAL_TIME: f32 = BEFORE_TIME + TRANSITION_TIME + WAIT_TIME;

    pub async fn new(info: ChartInfo, config: Config, mut fs: Box<dyn FileSystem>, get_size_fn: Option<Box<dyn Fn() -> (u32, u32)>>) -> Result<Self> {
        async fn load(fs: &mut Box<dyn FileSystem>, path: &str) -> Result<(Texture2D, Texture2D)> {
            let image = image::load_from_memory(&fs.load_file(path).await?).context("Failed to decode image")?;
            let mut blurred_rgb = image.to_rgb8();
            let size = blurred_rgb.width() as usize * blurred_rgb.height() as usize;
            let mut vec = unsafe { Vec::from_raw_parts(std::mem::transmute(blurred_rgb.as_mut_ptr()), size, size) };
            fastblur::gaussian_blur(&mut vec, image.width() as _, image.height() as _, 50.);
            std::mem::forget(vec);
            let mut blurred = ImageBuffer::<image::Rgba<u8>, _>::new(image.width(), image.height());
            for (input, output) in blurred_rgb.chunks_exact(3).zip(blurred.chunks_exact_mut(4)) {
                output[..3].copy_from_slice(input);
                output[3] = 255;
            }
            Ok((
                Texture2D::from_image(&Image {
                    width: image.width() as u16,
                    height: image.height() as u16,
                    bytes: image.into_rgba8().into_raw(),
                }),
                Texture2D::from_image(&Image {
                    width: blurred.width() as u16,
                    height: blurred.height() as u16,
                    bytes: blurred.into_raw(),
                }),
            ))
        }

        let background = match load(&mut fs, &info.illustration).await {
            Ok((ill, bg)) => Some((ill, bg)),
            Err(err) => {
                warn!("Failed to load background: {:?}", err);
                None
            }
        };
        let (illustration, background) =
            background.unwrap_or_else(|| (Texture2D::from_rgba8(1, 1, &[0, 0, 0, 1]), Texture2D::from_rgba8(1, 1, &[0, 0, 0, 1])));
        let font = match load_ttf_font("font.ttf").await {
            Err(err) => {
                warn!("Failed to load font, falling back to default\n{err:?}");
                Font::default()
            }
            Ok(font) => font,
        };
        let future = Box::pin(GameScene::new(info.clone(), config, fs, background, illustration, font, get_size_fn));
        Ok(Self {
            info,
            illustration,
            background,
            font,
            future: Some(future),
            next_scene: None,
            finish_time: f32::INFINITY,
            target: None,
        })
    }
}

impl Scene for LoadingScene {
    fn enter(&mut self, _tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        self.target = target;
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        if let Some(future) = self.future.as_mut() {
            let waker = waker();
            let mut futures_context = std::task::Context::from_waker(&waker);
            loop {
                match future.as_mut().poll(&mut futures_context) {
                    Poll::Pending => {
                        if self.target.is_none() {
                            break;
                        }
                    }
                    Poll::Ready(game_scene) => {
                        self.future = None;
                        self.next_scene = Some(Box::new(game_scene?));
                        self.finish_time = tm.now() as f32 + BEFORE_TIME;
                        break;
                    }
                }
            }
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager) -> Result<()> {
        let asp = screen_width() / screen_height();
        let top = 1. / asp;
        let now = tm.now() as f32;
        let intern = unsafe { get_internal_gl() };
        let gl = intern.quad_gl;
        set_camera(&Camera2D {
            zoom: vec2(1., -asp),
            render_target: self.target,
            ..Default::default()
        });
        draw_background(self.background);
        let dx = if now > self.finish_time {
            let p = ((now - self.finish_time) / TRANSITION_TIME).min(1.);
            p.powi(3) * 2.
        } else {
            0.
        };
        if dx != 0. {
            gl.push_model_matrix(Mat4::from_translation(vec3(dx, 0., 0.)));
        }
        let vo = -top / 10.;
        let r = draw_illustration(self.illustration, 0.38, vo, 1.);
        let h = r.h / 3.6;
        let main = Rect::new(-0.88, vo - h / 2. - top / 10., 0.78, h);
        draw_parallelogram(main, None, Color::new(0., 0., 0., 0.7));
        draw_text_aligned(
            self.font,
            &self.info.name,
            main.x + main.w * 0.09,
            main.y + main.h * 0.36,
            (0., 0.5),
            if self.info.name.len() > 9 { 0.6 } else { 0.84 },
            WHITE,
        );
        draw_text_aligned(self.font, &self.info.composer, main.x + main.w * 0.09, main.y + main.h * 0.73, (0., 0.5), 0.36, WHITE);

        let ext = 0.06;
        let sub = Rect::new(main.x + main.w * 0.71, main.y - main.h * ext, main.w * 0.26, main.h * (1. + ext * 2.));
        let mut ct = sub.center();
        ct.x += sub.w * 0.02;
        draw_parallelogram(sub, None, WHITE);
        draw_text_aligned(self.font, &(self.info.difficulty.round() as u32).to_string(), ct.x, ct.y + sub.h * 0.05, (0.5, 1.), 0.88, BLACK);
        draw_text_aligned(
            self.font,
            self.info.level.split_whitespace().next().unwrap_or_default(),
            ct.x,
            ct.y + sub.h * 0.09,
            (0.5, 0.),
            0.34,
            BLACK,
        );
        let t = draw_text_aligned(self.font, "Chart", main.x + main.w / 6., main.y + main.h * 1.27, (0., 0.), 0.3, WHITE);
        draw_text_aligned(self.font, &self.info.charter, t.x, t.y + top / 20., (0., 0.), 0.47, WHITE);
        let w = 0.027;
        let t = draw_text_aligned(self.font, "Illustration", t.x - w, t.y + w / 0.13 / 13. * 5., (0., 0.), 0.3, WHITE);
        draw_text_aligned(self.font, &self.info.illustrator, t.x, t.y + top / 20., (0., 0.), 0.47, WHITE);

        draw_text_aligned(self.font, &self.info.tip, -0.91, top * 0.92, (0., 1.), 0.47, WHITE);
        let t = draw_text_aligned(self.font, "Loading...", 0.87, top * 0.92, (1., 1.), 0.44, WHITE);
        let we = 0.2;
        let he = 0.5;
        let r = Rect::new(t.x - t.w * we, t.y - t.h * he, t.w * (1. + we * 2.), t.h * (1. + he * 2.));

        let p = 0.6;
        let s = 0.2;
        let t = ((now - 0.3).max(0.) % (p * 2. + s)) / p;
        let st = (t - 1.).max(0.).min(1.).powi(3);
        let en = 1. - (1. - t.min(1.)).powi(3);

        draw_rectangle(r.x + r.w * st, r.y, r.w * (en - st), r.h, WHITE);
        let lt = (r.x + r.w * st, r.y);
        let lt = ((lt.0 + 1. + dx) / 2. * screen_width(), lt.1 / 2. * screen_width() + screen_height() / 2.);
        gl.scissor(Some((lt.0 as _, lt.1 as _, (r.w * (en - st) * screen_width() / 2.).ceil() as _, (r.h * screen_width() / 2.).ceil() as _)));
        draw_text_aligned(self.font, "Loading...", 0.87, top * 0.92, (1., 1.), 0.44, BLACK);
        gl.scissor(None);

        if dx != 0. {
            gl.pop_model_matrix();
        }
        Ok(())
    }

    fn next_scene(&mut self, tm: &mut TimeManager) -> NextScene {
        if tm.now() as f32 > self.finish_time + TRANSITION_TIME + WAIT_TIME {
            if let Some(scene) = self.next_scene.take() {
                return NextScene::Replace(scene);
            }
        }
        NextScene::None
    }
}
