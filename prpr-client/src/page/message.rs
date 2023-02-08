prpr::tl_file!("message");

use super::{Page, SharedState};
use crate::{
    cloud::{Client, Message},
    get_data, get_data_mut, save_data,
};
use anyhow::Result;
use chrono::{Local, Utc};
use macroquad::prelude::*;
use prpr::{
    scene::show_error,
    task::Task,
    ui::{RectButton, Scroll, Ui},
};
use std::borrow::Cow;

pub struct MessagePage {
    list_scroll: Scroll,
    content_scroll: Scroll,
    load_task: Option<Task<Result<Vec<Message>>>>,

    messages: Vec<(Message, RectButton)>,
    focus: Option<usize>,
    has_new: bool,
}

impl MessagePage {
    pub fn new() -> Self {
        Self {
            list_scroll: Scroll::new(),
            content_scroll: Scroll::new(),
            load_task: Some(Task::new(Client::messages())),

            messages: Vec::new(),
            focus: None,
            has_new: false,
        }
    }
}

impl Page for MessagePage {
    fn label(&self) -> Cow<'static, str> {
        tl!("label")
    }
    fn has_new(&self) -> bool {
        self.has_new
    }

    fn update(&mut self, _focus: bool, state: &mut SharedState) -> Result<()> {
        let t = state.t;
        if self.list_scroll.y_scroller.pulled && self.load_task.is_none() {
            self.has_new = false;
            self.focus = None;
            self.messages.clear();
            self.load_task = Some(Task::new(Client::messages()));
        }
        self.list_scroll.update(t);
        self.content_scroll.update(t);
        if let Some(task) = self.load_task.as_mut() {
            if let Some(msgs) = task.take() {
                match msgs {
                    Ok(msgs) => {
                        self.has_new = msgs
                            .first()
                            .map_or(false, |it| get_data().message_check_time.map_or(true, |check| check < it.updated_at));
                        self.messages = msgs.into_iter().map(|it| (it, RectButton::new())).collect();
                    }
                    Err(err) => {
                        show_error(err.context(tl!("load-failed")));
                    }
                }
                self.load_task = None;
            }
        }
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, state: &mut super::SharedState) -> Result<bool> {
        let t = state.t;
        if self.list_scroll.touch(touch, t) {
            return Ok(true);
        }
        for (id, (_, btn)) in self.messages.iter_mut().enumerate() {
            if btn.touch(touch) {
                self.focus = if self.focus == Some(id) { None } else { Some(id) };
                self.content_scroll.y_scroller.set_offset(0.);
                if self.has_new {
                    get_data_mut().message_check_time = Some(Utc::now());
                    save_data()?;
                    self.has_new = false;
                }
                return Ok(true);
            }
        }
        if self.content_scroll.touch(touch, t) {
            return Ok(true);
        }
        Ok(false)
    }

    fn render(&mut self, ui: &mut Ui, state: &mut SharedState) -> Result<()> {
        let width = 0.4;
        self.list_scroll.size((width, state.content_size.1 - 0.01));
        ui.fill_rect(self.list_scroll.rect(), Color::new(0., 0., 0., 0.3));
        self.list_scroll.render(ui, |ui| {
            let pd = 0.02;
            let vpad = 0.015;
            let mut h = vpad;
            for (id, (msg, btn)) in self.messages.iter_mut().enumerate() {
                ui.dy(vpad);
                h += vpad;
                let r = Rect::new(pd, 0., width - pd * 2., 0.07);
                ui.fill_rect(r, Color::new(1., 1., 1., if Some(id) == self.focus { 0.3 } else { 0.5 }));
                ui.text(&msg.title)
                    .pos(r.x + 0.01, r.center().y)
                    .anchor(0., 0.5)
                    .no_baseline()
                    .size(0.4)
                    .max_width(r.w)
                    .draw();
                btn.set(ui, r);
                ui.dy(r.h);
                h += r.h;
            }
            (width, h)
        });
        let dx = width + 0.02;
        ui.dx(dx);
        let width = state.content_size.0 - dx;
        self.content_scroll.size((width, state.content_size.1 - 0.01));
        if let Some(focus) = self.focus {
            let msg = &self.messages[focus].0;
            ui.fill_rect(self.content_scroll.rect(), Color::new(0., 0., 0., 0.3));
            self.content_scroll.render(ui, |ui| {
                let mut h = 0.;
                let pd = 0.02;
                ui.dx(pd);
                macro_rules! dy {
                    ($dy:expr) => {{
                        let dy = $dy;
                        h += dy;
                        ui.dy(dy);
                    }};
                }
                dy!(0.02);
                let r = ui.text(&msg.title).size(0.9).draw();
                dy!(r.h + 0.02);
                ui.fill_rect(Rect::new(0., 0., width - pd * 2., 0.007), WHITE);
                dy!(0.007 + 0.01);
                let c = Color::new(1., 1., 1., 0.6);
                let r = ui.text(&msg.author).size(0.3).color(c).draw();
                let r = ui
                    .text(&tl!("updated", "time" => msg.updated_at.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string()))
                    .size(0.3)
                    .pos(r.w + 0.01, 0.)
                    .color(c)
                    .draw();
                dy!(r.h + 0.018);
                let r = ui.text(&msg.content).size(0.5).max_width(width - pd * 2.).multiline().draw();
                dy!(r.h + 0.027);
                (width, h)
            });
        }
        Ok(())
    }
}
