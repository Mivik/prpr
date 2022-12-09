use crate::judge::JudgeStatus;

use super::{JudgeLine, Resource};

#[derive(Default)]
pub struct Chart {
    pub offset: f32,
    pub lines: Vec<JudgeLine>,
}

impl Chart {
    pub fn reset(&mut self) {
        self
            .lines
            .iter_mut()
            .flat_map(|it| it.notes.iter_mut())
            .for_each(|note| note.judge = JudgeStatus::NotJudged);
        for line in &mut self.lines {
            line.cache.reset(&mut line.notes);
        }
    }

    pub fn update(&mut self, res: &mut Resource) {
        for line in &mut self.lines {
            line.update(res);
        }
    }

    pub fn render(&self, res: &mut Resource) {
        for line in &self.lines {
            line.render(res, &self.lines);
        }
    }
}
