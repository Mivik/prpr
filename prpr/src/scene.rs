mod ending;
pub use ending::EndingScene;

mod game;
pub use game::GameScene;

mod loading;
pub use loading::LoadingScene;

use crate::{
    ext::{draw_image, screen_aspect, ScaleType},
    time::TimeManager,
};
use anyhow::Result;
use macroquad::prelude::{
    utils::{register_input_subscriber, repeat_all_miniquad_input},
    *,
};
use miniquad::EventHandler;

#[derive(Default)]
pub enum NextScene {
    #[default]
    None,
    Pop,
    PopN(usize),
    Exit,
    Overlay(Box<dyn Scene>),
    Replace(Box<dyn Scene>),
}

pub trait Scene {
    fn enter(&mut self, _tm: &mut TimeManager, _target: Option<RenderTarget>) -> Result<()> {
        Ok(())
    }
    fn pause(&mut self, _tm: &mut TimeManager) -> Result<()> {
        Ok(())
    }
    fn resume(&mut self, _tm: &mut TimeManager) -> Result<()> {
        Ok(())
    }
    fn touch(&mut self, _tm: &mut TimeManager, _touch: Touch) -> Result<()> {
        Ok(())
    }
    fn update(&mut self, tm: &mut TimeManager) -> Result<()>;
    fn render(&mut self, tm: &mut TimeManager) -> Result<()>;
    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        NextScene::None
    }
}

pub struct Main {
    scenes: Vec<Box<dyn Scene>>,
    times: Vec<f64>,
    target: Option<RenderTarget>,
    tm: TimeManager,
    subscriber: usize,
    last_update_time: f64,
    should_exit: bool,
}

impl Main {
    pub fn new(mut scene: Box<dyn Scene>, mut tm: TimeManager, target: Option<RenderTarget>) -> Result<Self> {
        simulate_mouse_with_touch(false);
        scene.enter(&mut tm, target)?;
        let last_update_time = tm.now();
        Ok(Self {
            scenes: vec![scene],
            times: Vec::new(),
            target,
            tm,
            subscriber: register_input_subscriber(),
            last_update_time,
            should_exit: false,
        })
    }

    pub fn update(&mut self) -> Result<()> {
        match self.scenes.last_mut().unwrap().next_scene(&mut self.tm) {
            NextScene::None => {}
            NextScene::Pop => {
                self.scenes.pop();
                self.tm.seek_to(self.times.pop().unwrap());
                self.scenes.last_mut().unwrap().enter(&mut self.tm, self.target)?;
            }
            NextScene::PopN(num) => {
                for _ in 0..num {
                    self.scenes.pop();
                    self.tm.seek_to(self.times.pop().unwrap());
                }
                self.scenes.last_mut().unwrap().enter(&mut self.tm, self.target)?;
            }
            NextScene::Exit => {
                self.should_exit = true;
            }
            NextScene::Overlay(mut scene) => {
                self.times.push(self.tm.now());
                scene.enter(&mut self.tm, self.target)?;
                self.scenes.push(scene);
            }
            NextScene::Replace(mut scene) => {
                scene.enter(&mut self.tm, self.target)?;
                *self.scenes.last_mut().unwrap() = scene;
            }
        }
        let mut handler = Handler(Vec::new());
        repeat_all_miniquad_input(&mut handler, self.subscriber);
        if !handler.0.is_empty() {
            let now = self.tm.now();
            let delta = (now - self.last_update_time) / handler.0.len() as f64;
            let vp = unsafe { get_internal_gl() }.quad_gl.get_viewport();
            for (index, mut touch) in handler.0.into_iter().enumerate() {
                let Vec2 { x, y } = touch.position;
                touch.position =
                    vec2((x - vp.0 as f32) / vp.2 as f32 * 2. - 1., ((y - vp.1 as f32) / vp.3 as f32 * 2. - 1.) / (vp.2 as f32 / vp.3 as f32));
                self.tm.seek_to(self.last_update_time + (index + 1) as f64 * delta);
                self.scenes.last_mut().unwrap().touch(&mut self.tm, touch)?;
            }
            self.tm.seek_to(now);
        }
        self.last_update_time = self.tm.now();
        self.scenes.last_mut().unwrap().update(&mut self.tm)
    }

    pub fn render(&mut self) -> Result<()> {
        self.scenes.last_mut().unwrap().render(&mut self.tm)
    }

    pub fn pause(&mut self) -> Result<()> {
        self.scenes.last_mut().unwrap().pause(&mut self.tm)
    }

    pub fn resume(&mut self) -> Result<()> {
        self.scenes.last_mut().unwrap().resume(&mut self.tm)
    }

    pub fn should_exit(&self) -> bool {
        self.should_exit
    }
}

fn draw_background(tex: Texture2D) {
    let asp = screen_aspect();
    let top = 1. / asp;
    draw_image(tex, Rect::new(-1., -top, 2., top * 2.), ScaleType::Scale);
    draw_rectangle(-1., -top, 2., top * 2., Color::new(0., 0., 0., 0.3));
}

fn draw_illustration(tex: Texture2D, x: f32, y: f32, w: f32, h: f32, color: Color) -> Rect {
    let scale = 0.076;
    let w = scale * 13. * w;
    let h = scale * 7. * h;
    let r = Rect::new(x - w / 2., y - h / 2., w, h);
    let tr = {
        let exp = w / h;
        let act = tex.width() / tex.height();
        if exp > act {
            let h = act / exp;
            Rect::new(0., 0.5 - h / 2., 1., h)
        } else {
            let w = exp / act;
            Rect::new(0.5 - w / 2., 0., w, 1.)
        }
    };
    crate::ext::draw_parallelogram(r, Some((tex, tr)), color, true);
    r
}

struct Handler(Vec<Touch>);

fn button_to_id(button: MouseButton) -> u64 {
    u64::MAX
        - match button {
            MouseButton::Left => 0,
            MouseButton::Middle => 1,
            MouseButton::Right => 2,
            MouseButton::Unknown => 3,
        }
}

impl EventHandler for Handler {
    fn update(&mut self, _: &mut miniquad::Context) {}
    fn draw(&mut self, _: &mut miniquad::Context) {}
    fn touch_event(&mut self, _: &mut miniquad::Context, phase: miniquad::TouchPhase, id: u64, x: f32, y: f32) {
        self.0.push(Touch {
            id,
            phase: match phase {
                miniquad::TouchPhase::Started => TouchPhase::Started,
                miniquad::TouchPhase::Moved => TouchPhase::Moved,
                miniquad::TouchPhase::Ended => TouchPhase::Ended,
                miniquad::TouchPhase::Cancelled => TouchPhase::Cancelled,
            },
            position: vec2(x, y),
        });
    }

    fn mouse_button_down_event(&mut self, _ctx: &mut miniquad::Context, button: MouseButton, x: f32, y: f32) {
        self.0.push(Touch {
            id: button_to_id(button),
            phase: TouchPhase::Started,
            position: vec2(x, y),
        });
    }

    fn mouse_motion_event(&mut self, _ctx: &mut miniquad::Context, x: f32, y: f32) {
        if is_mouse_button_down(MouseButton::Left) {
            self.0.push(Touch {
                id: button_to_id(MouseButton::Left),
                phase: TouchPhase::Moved,
                position: vec2(x, y),
            });
        }
    }

    fn mouse_button_up_event(&mut self, _ctx: &mut miniquad::Context, button: MouseButton, x: f32, y: f32) {
        self.0.push(Touch {
            id: button_to_id(button),
            phase: TouchPhase::Ended,
            position: vec2(x, y),
        });
    }

    fn key_down_event(&mut self, _ctx: &mut miniquad::Context, _keycode: KeyCode, _keymods: miniquad::KeyMods, _repeat: bool) {}

    fn key_up_event(&mut self, _ctx: &mut miniquad::Context, _keycode: KeyCode, _keymods: miniquad::KeyMods) {}
}
