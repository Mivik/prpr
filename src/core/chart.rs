use super::{JudgeLine, Resource};

#[derive(Default)]
pub struct Chart {
    pub offset: f32,
    pub lines: Vec<JudgeLine>,
}

impl Chart {
    pub fn update(&mut self, res: &mut Resource) {
        for line in &mut self.lines {
            line.update(res);
        }
    }

    pub fn render(&self, res: &mut Resource) {
        for line in &self.lines {
            line.render(res);
        }
    }
}
