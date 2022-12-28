use super::{Effect, JudgeLine, Matrix, Resource, UIElement, Vector};
use crate::{judge::JudgeStatus, ui::Ui};

#[derive(Default)]
pub struct Chart {
    pub offset: f32,
    pub lines: Vec<JudgeLine>,
    pub effects: Vec<Effect>,
    pub order: Vec<usize>,
    pub attach_ui: [Option<usize>; 7],
}

impl Chart {
    pub fn new(offset: f32, lines: Vec<JudgeLine>, effects: Vec<Effect>) -> Self {
        let mut attach_ui = [None; 7];
        let mut order = (0..lines.len())
            .filter(|it| {
                if let Some(element) = lines[*it].attach_ui {
                    attach_ui[element as usize] = Some(*it);
                    false
                } else {
                    true
                }
            })
            .collect::<Vec<_>>();
        order.sort_by_key(|it| (lines[*it].z_index, *it));
        Self {
            offset,
            lines,
            effects,
            order,
            attach_ui,
        }
    }

    #[inline]
    pub fn with_element(&self, ui: &mut Ui, res: &Resource, element: UIElement, f: impl FnOnce(&mut Ui, f32, Matrix)) {
        if let Some(id) = self.attach_ui[element as usize] {
            let obj = &self.lines[id].object;
            ui.with(obj.now(res), |ui| f(ui, obj.now_alpha(), obj.now_scale()));
        } else {
            f(ui, 1., Matrix::identity());
        }
    }

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
        res.apply_model_of(&Matrix::identity().append_nonuniform_scaling(&Vector::new(1.0, -1.0)), |res| {
            for id in &self.order {
                self.lines[*id].render(res, &self.lines);
            }
            for effect in &self.effects {
                effect.render(res);
            }
        });
    }
}
