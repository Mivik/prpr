use std::cell::RefCell;

use super::{BpmList, Effect, JudgeLine, Matrix, Resource, UIElement, Vector};
use crate::{judge::JudgeStatus, ui::Ui};
use macroquad::prelude::*;

#[derive(Default)]
pub struct ChartSettings {
    pub pe_alpha_extension: bool,
    pub hold_partial_cover: bool,
}

pub struct Chart {
    pub offset: f32,
    pub lines: Vec<JudgeLine>,
    pub bpm_list: RefCell<BpmList>,
    pub settings: ChartSettings,

    pub effects: Vec<Effect>,
    pub global_effects: Vec<Effect>,
    pub order: Vec<usize>,
    pub attach_ui: [Option<usize>; 7],
}

impl Chart {
    pub fn new(offset: f32, lines: Vec<JudgeLine>, bpm_list: BpmList, effects: Vec<Effect>, settings: ChartSettings) -> Self {
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
        let (global_effects, effects) = effects.into_iter().partition(|e| e.global);
        Self {
            offset,
            lines,
            bpm_list: RefCell::new(bpm_list),
            settings,

            effects,
            global_effects,
            order,
            attach_ui,
        }
    }

    #[inline]
    pub fn with_element(&self, ui: &mut Ui, res: &Resource, element: UIElement, f: impl FnOnce(&mut Ui, f32, Matrix)) {
        if let Some(id) = self.attach_ui[element as usize] {
            let obj = &self.lines[id].object;
            let mut tr = obj.now_translation(res);
            tr.y = -tr.y;
            ui.with(obj.now_rotation().append_translation(&tr), |ui| f(ui, obj.now_alpha(), obj.now_scale()));
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
            line.object.set_time(res.time);
        }
        // TODO optimize
        let trs = self.lines.iter().map(|it| it.now_transform(res, &self.lines)).collect::<Vec<_>>();
        for (line, tr) in self.lines.iter_mut().zip(trs) {
            line.update(res, tr);
        }
        for effect in &mut self.effects {
            effect.update(res);
        }
    }

    pub fn render(&self, res: &mut Resource) {
        res.apply_model_of(&Matrix::identity().append_nonuniform_scaling(&Vector::new(1.0, -1.0)), |res| {
            let mut guard = self.bpm_list.borrow_mut();
            for id in &self.order {
                self.lines[*id].render(res, &self.lines, &mut guard, &self.settings);
            }
            drop(guard);
            res.note_buffer.borrow_mut().draw_all();
            if res.config.sample_count > 1 {
                unsafe { get_internal_gl() }.flush();
                if let Some(target) = &res.chart_target {
                    target.blit();
                }
            }
            if !res.no_effect {
                for effect in &self.effects {
                    effect.render(res);
                }
            }
        });
    }
}
