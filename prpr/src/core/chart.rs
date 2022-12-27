use super::{JudgeLine, Resource, Effect};
use crate::judge::JudgeStatus;

#[derive(Default)]
pub struct Chart {
    pub offset: f32,
    pub lines: Vec<JudgeLine>,
    pub effects: Vec<Effect>,
}

impl Chart {
    pub fn reset(&mut self) {
        self.lines
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
        for effect in &mut self.effects {
            effect.update(res);
        }
    }

    pub fn render(&self, res: &mut Resource) {
        for line in &self.lines {
            line.render(res, &self.lines);
        }
        for effect in &self.effects {
            effect.render(res);
        }
    }
}
