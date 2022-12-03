use super::{JudgeLine, Resource};

#[derive(Default)]
pub struct Chart {
    pub offset: f32,
    pub lines: Vec<JudgeLine>,
}

impl Chart {
    pub fn set_time(&mut self, time: f32) {
        for line in &mut self.lines {
            line.set_time(time);
        }
    }

    pub fn render(&self, res: &Resource) {
        for line in &self.lines {
            line.render(res);
        }
    }
}
