mod ending;
pub use ending::EndingScene;

mod game;
pub use game::GameScene;

mod loading;
pub use loading::LoadingScene;

use crate::time::TimeManager;
use anyhow::Result;
use macroquad::prelude::*;

pub enum NextScene {
    None,
    Pop,
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
    fn update(&mut self, tm: &mut TimeManager) -> Result<()>;
    fn render(&mut self, tm: &mut TimeManager) -> Result<()>;
    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        NextScene::None
    }
}

pub struct Main {
    scenes: Vec<Box<dyn Scene>>,
    target: Option<RenderTarget>,
    tm: TimeManager,
    should_exit: bool,
}

impl Main {
    pub fn new(mut scene: Box<dyn Scene>, mut tm: TimeManager, target: Option<RenderTarget>) -> Result<Self> {
        scene.enter(&mut tm, target)?;
        Ok(Self {
            scenes: vec![scene],
            target,
            tm,
            should_exit: false,
        })
    }

    pub fn update(&mut self) -> Result<()> {
        match self.scenes.last_mut().unwrap().next_scene(&mut self.tm) {
            NextScene::None => {}
            NextScene::Pop => {
                self.scenes.pop();
                self.scenes.last_mut().unwrap().enter(&mut self.tm, self.target)?;
            }
            NextScene::Exit => {
                self.should_exit = true;
            }
            NextScene::Overlay(mut scene) => {
                scene.enter(&mut self.tm, self.target)?;
                self.scenes.push(scene);
            }
            NextScene::Replace(mut scene) => {
                scene.enter(&mut self.tm, self.target)?;
                *self.scenes.last_mut().unwrap() = scene;
            }
        }
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
    let asp = screen_width() / screen_height();
    let top = 1. / asp;
    let bw = tex.width();
    let bh = tex.height();
    let s = (2. / bw).max(2. / bh / asp);
    draw_texture_ex(
        tex,
        -bw * s / 2.,
        -bh * s / 2.,
        WHITE,
        DrawTextureParams {
            dest_size: Some(vec2(bw * s, bh * s)),
            ..Default::default()
        },
    );
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
