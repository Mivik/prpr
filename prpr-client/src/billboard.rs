use prpr::ui::Ui;
use std::collections::VecDeque;

pub const MAX_SIZE: usize = 7;
pub const LAST_TIME: f32 = 2.;
pub const PADDING: f32 = 0.02;

pub struct BillBoard {
    messages: VecDeque<(String, f32)>,
}

impl BillBoard {
    pub fn new() -> Self {
        Self { messages: VecDeque::new() }
    }

    pub fn render(&self, ui: &mut Ui) {
        let rt = 1. - PADDING;
        let mut tp = -ui.top + PADDING;
        for msg in &self.messages {
            let r = ui.text(&msg.0).pos(rt, tp).size(0.8).anchor(1., 0.).draw();
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

    pub fn update(&mut self, t: f32) {
        while let Some(front) = self.messages.front() {
            if t > front.1 + LAST_TIME {
                self.messages.pop_front();
            } else {
                break;
            }
        }
    }
}
