use macroquad::prelude::Color;

use crate::{ui::Ui, ext::RectExt};
use std::collections::VecDeque;

pub const MAX_SIZE: usize = 7;
pub const LAST_TIME: f32 = 2.;
pub const PADDING: f32 = 0.02;

pub struct BillBoard {
    messages: VecDeque<(String, f32)>,
}

impl Default for BillBoard {
    fn default() -> Self {
        Self::new()
    }
}

impl BillBoard {
    pub fn new() -> Self {
        Self { messages: VecDeque::new() }
    }

    pub fn render(&mut self, ui: &mut Ui, t: f32) {
        while let Some(front) = self.messages.front() {
            if t > front.1 + LAST_TIME {
                self.messages.pop_front();
            } else {
                break;
            }
        }
        let rt = 1. - PADDING;
        let mut tp = -ui.top + PADDING;
        for msg in &self.messages {
            let text = ui.text(&msg.0).pos(rt, tp).size(0.8).anchor(1., 0.);
            let r = text.measure();
            text.ui.fill_rect(r.feather(0.01), Color::new(0., 0., 0., 0.3));
            text.draw();
            tp += r.h + 0.02;
        }
    }

    pub fn clear(&mut self) {
        self.messages.clear();
    }

    pub fn add(&mut self, msg: impl Into<String>, t: f32) {
        self.messages.push_back((msg.into(), t));
        if self.messages.len() > MAX_SIZE {
            self.messages.pop_front();
        }
    }
}
