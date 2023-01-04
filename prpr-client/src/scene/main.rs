use super::{song::TrashBin, SongScene};
use crate::{
    cloud::{Client, Images, LCChartItem, User, UserManager},
    data::{BriefChartInfo, LocalChart},
    dir, get_data, get_data_mut, save_data,
    task::Task,
};
use anyhow::{Context, Result};
use image::{imageops::FilterType, DynamicImage};
use lyon::{
    math as lm,
    path::{builder::BorderRadii, Path, Winding},
};
use macroquad::{prelude::*, texture::RenderTarget};
use once_cell::sync::Lazy;
use prpr::{
    audio::{Audio, AudioClip, AudioHandle, DefaultAudio, PlayParams},
    config::ChallengeModeColor,
    core::{ParticleEmitter, SkinPack, Tweenable, JUDGE_LINE_PERFECT_COLOR, NOTE_WIDTH_RATIO_BASE},
    ext::{poll_future, screen_aspect, LocalTask, RectExt, SafeTexture, ScaleType, BLACK_TEXTURE},
    fs,
    scene::{request_file, request_input, return_file, return_input, show_error, show_message, take_file, take_input, NextScene, Scene},
    time::TimeManager,
    ui::{RectButton, Scroll, Ui},
};
use regex::Regex;
use serde_json::json;
use std::{
    collections::HashMap,
    future::Future,
    io::Cursor,
    ops::DerefMut,
    pin::Pin,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Mutex,
    },
};
use tempfile::NamedTempFile;

const SIDE_PADDING: f32 = 0.02;
const ROW_NUM: u32 = 4;
const CARD_HEIGHT: f32 = 0.3;
const CARD_PADDING: f32 = 0.02;

const SWITCH_TIME: f32 = 0.4;
const TRANSIT_TIME: f32 = 0.4;

pub static SHOULD_DELETE: AtomicBool = AtomicBool::new(false);
pub static UPDATE_TEXTURE: Mutex<Option<SafeTexture>> = Mutex::new(None);
pub static UPDATE_INFO: AtomicBool = AtomicBool::new(false);
pub static TRANSIT_ID: AtomicU32 = AtomicU32::new(0);

pub fn illustration_task(path: String) -> Task<Result<DynamicImage>> {
    Task::new(async move {
        let mut fs = fs::fs_from_file(std::path::Path::new(&format!("{}/{}", dir::charts()?, path)))?;
        let info = fs::load_info(fs.deref_mut()).await?;
        Ok(image::load_from_memory(&fs.load_file(&info.illustration).await?)?)
    })
}

fn load_local(tex: &SafeTexture) -> Vec<ChartItem> {
    get_data()
        .charts
        .iter()
        .rev()
        .map(|it| ChartItem {
            info: it.info.clone(),
            path: it.path.clone(),
            illustration: tex.clone(),
            illustration_task: Some(illustration_task(it.path.clone())),
        })
        .collect()
}

fn validate_username(username: &str) -> Option<&'static str> {
    if !(4..=20).contains(&username.len()) {
        return Some("用户名长度应介于 4-20 之间");
    }
    if username.chars().any(|it| it != '_' && it != '-' && !it.is_alphanumeric()) {
        return Some("用户名包含非法字符");
    }
    None
}

pub struct ChartItem {
    pub info: BriefChartInfo,
    pub path: String,
    pub illustration: SafeTexture,
    pub illustration_task: Option<Task<Result<DynamicImage>>>,
}

pub struct AccountPage {
    register: bool,
    task: Option<Task<Result<Option<User>>>>,
    task_desc: String,
    email_input: String,
    username_input: String,
    password_input: String,
    avatar_button: RectButton,
}

impl AccountPage {
    pub fn new() -> Self {
        let logged_in = get_data().me.is_some();
        Self {
            register: false,
            task: if logged_in {
                Some(Task::new(async { Ok(Some(Client::get_me().await?)) }))
            } else {
                None
            },
            task_desc: if logged_in { "更新数据".to_owned() } else { String::new() },
            email_input: String::new(),
            username_input: String::new(),
            password_input: String::new(),
            avatar_button: RectButton::new(),
        }
    }

    pub fn start(&mut self, desc: impl Into<String>, future: impl Future<Output = Result<Option<User>>> + Send + 'static) {
        self.task_desc = desc.into();
        self.task = Some(Task::new(future));
    }
}

pub struct MainScene {
    target: Option<RenderTarget>,
    next_scene: Option<NextScene>,
    scroll_local: Scroll,
    scroll_remote: Scroll,
    tex: SafeTexture,
    click_texture: SafeTexture,
    icon_back: SafeTexture,
    icon_play: SafeTexture,
    icon_edit: SafeTexture,
    icon_delete: SafeTexture,
    icon_question: SafeTexture,
    _skin: SkinPack,

    audio: DefaultAudio,
    cali_clip: AudioClip,
    cali_hit_clip: AudioClip,
    cali_handle: Option<AudioHandle>,
    cali_tm: TimeManager,
    cali_last: bool,
    emitter: ParticleEmitter,

    task_load: Task<Result<Vec<ChartItem>>>,
    remote_first_time: bool,
    loading_remote: bool,
    charts_local: Vec<ChartItem>,
    charts_remote: Vec<ChartItem>,

    choose_local: Option<u32>,
    choose_remote: Option<u32>,

    tab_scroll: Scroll,
    tab_index: usize,
    tab_buttons: [RectButton; 5],
    tab_start_time: f32,
    tab_from_index: usize,

    import_button: RectButton,
    import_task: Task<Result<LocalChart>>,
    load_skin_task: LocalTask<Result<(SkinPack, Option<String>)>>,

    account_page: AccountPage,

    chal_buttons: [RectButton; 6],

    downloading: HashMap<String, (String, Task<Result<LocalChart>>)>,
    transit: Option<(u32, f32, Rect, bool)>,
}

impl MainScene {
    pub async fn new() -> Result<Self> {
        let tex: SafeTexture = Texture2D::from_image(&load_image("player.jpg").await?).into();
        let audio = DefaultAudio::new()?;
        let cali_clip = audio.create_clip(load_file("cali.ogg").await?)?.0;
        let cali_hit_clip = audio.create_clip(load_file("cali_hit.ogg").await?)?.0;

        let mut cali_tm = TimeManager::new(1., true);
        cali_tm.force = 3e-2;
        macro_rules! load_tex {
            ($path:literal) => {
                SafeTexture::from(Texture2D::from_image(&load_image($path).await?))
            };
        }
        if let Some(user) = &get_data().me {
            UserManager::request(&user.id);
        }
        let skin = SkinPack::load(fs::fs_from_assets("skin/")?.deref_mut()).await?;
        let emitter = ParticleEmitter::new(&skin, get_data().config.note_scale)?;
        Ok(Self {
            target: None,
            next_scene: None,
            scroll_local: Scroll::new(),
            scroll_remote: Scroll::new(),
            tex: tex.clone(),
            click_texture: skin.note_style.click.clone(),
            icon_back: load_tex!("back.png"),
            icon_play: load_tex!("resume.png"),
            icon_edit: load_tex!("edit.png"),
            icon_delete: load_tex!("delete.png"),
            icon_question: load_tex!("question.png"),
            _skin: skin,

            audio,
            cali_clip,
            cali_hit_clip,
            cali_handle: None,
            cali_tm,
            cali_last: false,
            emitter,

            task_load: Task::pending(),
            remote_first_time: true,
            loading_remote: false,
            charts_local: load_local(&tex),
            charts_remote: Vec::new(),

            choose_local: None,
            choose_remote: None,

            tab_scroll: Scroll::new(),
            tab_index: 0,
            tab_buttons: [RectButton::new(); 5],
            tab_start_time: f32::NEG_INFINITY,
            tab_from_index: 0,

            import_button: RectButton::new(),
            import_task: Task::pending(),
            load_skin_task: None,

            account_page: AccountPage::new(),

            chal_buttons: [RectButton::new(); 6],

            downloading: HashMap::new(),
            transit: None,
        })
    }

    fn render_scroll(ui: &mut Ui, content_size: (f32, f32), scroll: &mut Scroll, charts: &mut Vec<ChartItem>) {
        scroll.size(content_size);
        scroll.render(ui, |ui| {
            let cw = content_size.0 / ROW_NUM as f32;
            let ch = CARD_HEIGHT;
            let p = CARD_PADDING;
            let path = {
                let mut path = Path::builder();
                path.add_rounded_rectangle(&lm::Box2D::new(lm::point(p, p), lm::point(cw - p, ch - p)), &BorderRadii::new(0.01), Winding::Positive);
                path.build()
            };
            ui.hgrids(content_size.0, ch, ROW_NUM, charts.len() as u32, |ui, id| {
                let chart = &mut charts[id as usize];
                if let Some(task) = &mut chart.illustration_task {
                    if let Some(image) = task.take() {
                        chart.illustration = if let Ok(image) = image { image.into() } else { BLACK_TEXTURE.clone() };
                        chart.illustration_task = None;
                    }
                }
                ui.fill_path(&path, (*chart.illustration, Rect::new(0., 0., cw, ch)));
                ui.fill_path(&path, Color::new(0., 0., 0., 0.55));
                ui.text(&chart.info.name)
                    .pos(p + 0.01, ch - p - 0.02)
                    .max_width(cw - p * 2.)
                    .anchor(0., 1.)
                    .size(0.6)
                    .draw();
            })
        });
    }

    fn ui(&mut self, ui: &mut Ui, t: f32) {
        let px = SIDE_PADDING;
        ui.scope(|ui| {
            ui.dx(-1. + px);
            ui.dy(-ui.top + 0.03);
            let mut dx = 0.;
            let mut max_height: f32 = 0.;
            let mut from_range = (0., 0.);
            let mut current_range = (0., 0.);
            for (id, tab) in ["本地", "在线", "账户", "设置", "关于"].into_iter().enumerate() {
                let r = ui.text(tab).pos(dx, 0.).size(0.9).draw();
                self.tab_buttons[id].set(ui, Rect::new(r.x, r.y, r.w, r.h + 0.01));
                max_height = max_height.max(r.h);
                let range = (dx, dx + r.w);
                if self.tab_from_index == id {
                    from_range = range;
                }
                if self.tab_index == id {
                    current_range = range;
                }
                dx += r.w + 0.02;
            }
            let draw_range = if t >= self.tab_start_time + SWITCH_TIME {
                current_range
            } else {
                let p = (t - self.tab_start_time) / SWITCH_TIME;
                let p = 1. - (1. - p).powi(3);
                (f32::tween(&from_range.0, &current_range.0, p), f32::tween(&from_range.1, &current_range.1, p))
            };
            ui.fill_rect(Rect::new(draw_range.0, max_height + 0.02, draw_range.1 - draw_range.0, 0.01), WHITE);
            ui.dy(max_height + 0.04);
            let pos = ui.to_global((0., 0.)).1;
            let width = (1. - px) * 2.;
            let content_size = (width, ui.top - pos - 0.01);
            self.tab_scroll.size(content_size);
            self.tab_scroll.render(ui, |ui| {
                Self::render_scroll(ui, content_size, &mut self.scroll_local, &mut self.charts_local);
                if let Some((id, _, rect, _)) = &mut self.transit {
                    *rect = ui.rect_to_global(Rect::new(
                        (*id % ROW_NUM) as f32 * width / ROW_NUM as f32,
                        (*id / ROW_NUM) as f32 * CARD_HEIGHT - self.scroll_local.y_scroller.offset(),
                        width / ROW_NUM as f32,
                        CARD_HEIGHT,
                    ));
                }
                {
                    let pad = 0.03;
                    let rad = 0.06;
                    let r = Rect::new(content_size.0 - pad - rad * 2., content_size.1 - pad - rad * 2., rad * 2., rad * 2.);
                    let ct = r.center();
                    ui.fill_circle(ct.x, ct.y, rad, ui.accent());
                    self.import_button.set(ui, r);
                    ui.text("+").pos(ct.x, ct.y).anchor(0.5, 0.5).size(1.4).no_baseline().draw();
                }
                ui.dx(content_size.0);
                Self::render_scroll(ui, content_size, &mut self.scroll_remote, &mut self.charts_remote);
                ui.dx(content_size.0);
                ui.scope(|ui| Self::render_account(ui, &mut self.account_page));
                ui.dx(content_size.0);
                if ui.scope(|ui| {
                    Self::render_settings(
                        ui,
                        &self.click_texture,
                        self.cali_tm.now() as _,
                        &mut self.cali_last,
                        &mut self.emitter,
                        &mut self.chal_buttons,
                        &mut self.load_skin_task,
                    )
                }) && self.tab_index == 3
                {
                    let _ = self.audio.play(&self.cali_hit_clip, PlayParams::default());
                }
                ui.dx(content_size.0);
                ui.scope(Self::render_about);
                (content_size.0 * 3., content_size.1)
            });
        });
    }

    fn render_about(ui: &mut Ui) {
        static ABOUT: Lazy<String> = Lazy::new(|| {
            String::from_utf8(base64::decode("cHJwciDmmK/kuIDmrL4gUGhpZ3JvcyDmqKHmi5/lmajvvIzml6jlnKjkuLroh6rliLbosLHmuLjnjqnmj5DkvpvkuIDkuKrnu5/kuIDljJbnmoTlubPlj7DjgILor7foh6rop4npgbXlrojnpL7nvqTnm7jlhbPopoHmsYLvvIzkuI3mgbbmhI/kvb/nlKggcHJwcu+8jOS4jemaj+aEj+WItuS9nOaIluS8oOaSreS9jui0qOmHj+S9nOWTgeOAggoKcHJwciDmmK/lvIDmupDova/ku7bvvIzpgbXlvqogR05VIEdlbmVyYWwgUHVibGljIExpY2Vuc2UgdjMuMCDljY/orq7jgIIK5rWL6K+V576k77yaNjYwNDg4Mzk2CkdpdEh1YjogaHR0cHM6Ly9naXRodWIuY29tL01pdmlrL3BycHI=").unwrap()).unwrap()
        });
        ui.dx(0.02);
        ui.dy(0.01);
        ui.text(&*ABOUT).multiline().max_width((1. - SIDE_PADDING) * 2. - 0.02).size(0.5).draw();
    }

    fn render_account(ui: &mut Ui, page: &mut AccountPage) {
        ui.dx(0.02);
        let r = Rect::new(0., 0., 0.22, 0.22);
        page.avatar_button.set(ui, r);
        if let Some(avatar) = get_data().me.as_ref().and_then(|it| UserManager::get_avatar(&it.id)) {
            let ct = r.center();
            ui.fill_circle(ct.x, ct.y, r.w / 2., (*avatar, r));
        }
        ui.text(get_data().me.as_ref().map(|it| it.name.as_str()).unwrap_or("[尚未登录]"))
            .pos(r.right() + 0.02, r.center().y)
            .anchor(0., 0.5)
            .size(0.8)
            .draw();
        ui.dy(r.h + 0.03);
        if get_data().me.is_none() {
            let r = ui.text("用户名").size(0.4).measure();
            ui.dx(r.w);
            if page.register {
                let r = ui.input("邮箱", &mut page.email_input, ());
                ui.dy(r.h + 0.02);
            }
            let r = ui.input("用户名", &mut page.username_input, ());
            ui.dy(r.h + 0.02);
            let r = ui.input("密码", &mut page.password_input, true);
            ui.dy(r.h + 0.02);
            let labels = if page.register {
                ["返回", if page.task.is_none() { "注册" } else { "注册中…" }]
            } else {
                ["注册", if page.task.is_none() { "登录" } else { "登录中…" }]
            };
            let cx = r.right() / 2.;
            let mut r = Rect::new(0., 0., cx - 0.01, r.h);
            if ui.button("left", r, labels[0]) {
                page.register ^= true;
            }
            r.x = cx + 0.01;
            if ui.button("right", r, labels[1]) {
                fn login(page: &mut AccountPage) -> Option<&'static str> {
                    let username = page.username_input.clone();
                    let password = page.password_input.clone();
                    if let Some(error) = validate_username(&username) {
                        return Some(error);
                    }
                    if !(6..=26).contains(&password.len()) {
                        return Some("密码长度应介于 6-26 之间");
                    }
                    if page.register {
                        let email = page.email_input.clone();
                        static EMAIL_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[\w\-\.]+@([\w\-]+\.)+[\w\-]{2,4}$").unwrap());
                        if !EMAIL_REGEX.is_match(&email) {
                            return Some("邮箱不合法");
                        }
                        page.start("注册", async move {
                            Client::register(&email, &username, &password).await?;
                            Ok(None)
                        });
                    } else {
                        page.start("登录", async move {
                            let user = Client::login(&username, &password).await?;
                            Ok(Some(user))
                        });
                    }
                    None
                }
                if let Some(err) = login(page) {
                    show_message(err);
                }
            }
        } else {
            let cx = 0.2;
            let mut r = Rect::new(0., 0., cx - 0.01, ui.text("呃").size(0.42).measure().h + 0.02);
            if ui.button("logout", r, "退出登录") {
                get_data_mut().me = None;
                let _ = save_data();
                show_message("退出登录成功");
            }
            r.x = cx + 0.01;
            if ui.button("edit_name", r, "修改名称") {
                request_input("edit_username", &get_data().me.as_ref().unwrap().name);
            }
        }
    }

    fn render_settings(
        ui: &mut Ui,
        click: &SafeTexture,
        cali_t: f32,
        cali_last: &mut bool,
        emitter: &mut ParticleEmitter,
        chal_buttons: &mut [RectButton; 6],
        skin_task: &mut LocalTask<Result<(SkinPack, Option<String>)>>,
    ) -> bool {
        let config = &mut get_data_mut().config;
        let s = 0.01;
        let mut result = false;
        ui.scope(|ui| {
            ui.dx(0.02);
            ui.scope(|ui| {
                let r = ui.checkbox("自动游玩", &mut config.autoplay);
                ui.dy(r.h + s);
                let r = ui.checkbox("双押提示", &mut config.multiple_hint);
                ui.dy(r.h + s);
                let r = ui.checkbox("固定宽高比", &mut config.fix_aspect_ratio);
                ui.dy(r.h + s);
                let r = ui.checkbox("自动对齐时间", &mut config.adjust_time);
                ui.dy(r.h + s);
                let r = ui.checkbox("粒子效果", &mut config.particle);
                ui.dy(r.h + s);
                let r = ui.checkbox("激进优化", &mut config.aggressive);
                ui.dy(r.h + s);
                let r = ui.slider("玩家 RKS", 1.0..17.0, 0.01, &mut config.player_rks, Some(0.45));
                ui.dy(r.h + s);
            });
            ui.dx(0.62);

            ui.scope(|ui| {
                let r = ui.slider("偏移(s)", -0.5..0.5, 0.005, &mut config.offset, None);
                ui.dy(r.h + s);
                let r = ui.slider("速度", 0.5..2.0, 0.005, &mut config.speed, None);
                ui.dy(r.h + s);
                let r = ui.slider("音符大小", 0.8..1.2, 0.005, &mut config.note_scale, None);
                emitter.set_scale(config.note_scale);
                ui.dy(r.h + s);
                let r = ui.slider("音乐音量", 0.0..2.0, 0.05, &mut config.volume_music, None);
                ui.dy(r.h + s);
                let r = ui.slider("音效音量", 0.0..2.0, 0.05, &mut config.volume_sfx, None);
                ui.dy(r.h + s);
                let r = ui.text("挑战模式颜色").size(0.4).draw();
                let chosen = config.challenge_color.clone() as usize;
                ui.dy(r.h + s * 2.);
                let dy = ui.scope(|ui| {
                    let mut max: f32 = 0.;
                    for (id, (name, button)) in ["白", "绿", "蓝", "红", "金", "彩"].into_iter().zip(chal_buttons.iter_mut()).enumerate() {
                        let r = ui.text(name).size(0.4).measure().feather(0.01);
                        button.set(ui, r);
                        ui.fill_rect(r, if chosen == id { ui.accent() } else { WHITE });
                        let color = if chosen == id { WHITE } else { ui.accent() };
                        ui.text(name).size(0.4).color(color).draw();
                        ui.dx(r.w + s);
                        max = max.max(r.h);
                    }
                    max
                });
                ui.dy(dy + s);

                let mut rks = config.challenge_rank as f32;
                let r = ui.slider("挑战模式等级", 0.0..48.0, 1., &mut rks, Some(0.45));
                config.challenge_rank = rks.round() as u32;
                ui.dy(r.h + s);
            });

            ui.scope(|ui| {
                ui.dx(0.65);
                let r = ui.text("皮肤").size(0.4).anchor(1., 0.).draw();
                let mut r = Rect::new(0.02, r.y - 0.01, 0.3, r.h + 0.02);
                if ui.button("choose_skin", r, config.skin_path.as_ref().map(|it| it.as_str()).unwrap_or("[默认]")) {
                    request_file("skin");
                }
                r.x += 0.3 + 0.02;
                r.w = 0.1;
                if ui.button("reset_skin", r, "重置") {
                    *skin_task = Some(Self::new_skin_task(None));
                }
            });

            let ct = (0.9, ui.top * 1.3);
            let len = 0.25;
            ui.fill_rect(Rect::new(ct.0 - len, ct.1 - 0.005, len * 2., 0.01), WHITE);
            let mut cali_t = cali_t - config.offset;
            if cali_t < 0. {
                cali_t += 2.;
            }
            if cali_t >= 2. {
                cali_t -= 2.;
            }
            if cali_t <= 1. {
                let w = NOTE_WIDTH_RATIO_BASE * config.note_scale * 2.;
                let h = w * click.height() / click.width();
                let r = Rect::new(ct.0 - w / 2., ct.1 + (cali_t - 1.) * 0.4, w, h);
                ui.fill_rect(r, (**click, r));
                *cali_last = true;
            } else {
                if *cali_last {
                    let g = ui.to_global(ct);
                    emitter.emit_at(vec2(g.0, g.1), JUDGE_LINE_PERFECT_COLOR);
                    result = true;
                }
                *cali_last = false;
            }
        });
        emitter.draw(get_frame_time());
        result
    }

    fn get_touched(pos: (f32, f32)) -> Option<u32> {
        let row = (pos.1 / CARD_HEIGHT) as i32;
        if row < 0 {
            return None;
        }
        let width = (2. - SIDE_PADDING * 2.) / ROW_NUM as f32;
        let column = (pos.0 / width) as i32;
        if column < 0 || column >= ROW_NUM as i32 {
            return None;
        }
        let x = pos.0 - width * column as f32;
        if x < CARD_PADDING || x + CARD_PADDING >= width {
            return None;
        }
        let y = pos.1 - CARD_HEIGHT * row as f32;
        if y < CARD_PADDING || y + CARD_PADDING >= CARD_HEIGHT {
            return None;
        }
        let id = row as u32 * ROW_NUM + column as u32;
        Some(id)
    }

    fn trigger_grid(phase: TouchPhase, choose: &mut Option<u32>, id: Option<u32>) -> bool {
        match phase {
            TouchPhase::Started => {
                *choose = id;
                false
            }
            TouchPhase::Moved | TouchPhase::Stationary => {
                if *choose != id {
                    *choose = None;
                }
                false
            }
            TouchPhase::Cancelled => {
                *choose = None;
                false
            }
            TouchPhase::Ended => choose.take() == id && id.is_some(),
        }
    }

    fn refresh_remote(&mut self) {
        if self.loading_remote {
            return;
        }
        self.charts_remote.clear();
        show_message("正在加载");
        self.loading_remote = true;
        self.task_load = Task::new({
            let tex = self.tex.clone();
            async move {
                let charts: Vec<LCChartItem> = Client::query().order("-updatedAt").send().await?;
                Ok(charts
                    .into_iter()
                    .map(|it| {
                        let illu = it.illustration;
                        ChartItem {
                            info: BriefChartInfo {
                                id: it.id,
                                ..it.info.clone()
                            },
                            path: it.file.url,
                            illustration: tex.clone(),
                            illustration_task: Some(Task::new(async move { Images::load(&illu).await })),
                        }
                    })
                    .collect::<Vec<_>>())
            }
        });
    }

    fn new_skin_task(path: Option<String>) -> Pin<Box<dyn Future<Output = Result<(SkinPack, Option<String>)>>>> {
        Box::pin(async move {
            let skin = SkinPack::from_path(path.as_ref()).await?;
            Ok((
                skin,
                if let Some(path) = path {
                    let dst = format!("{}/chart.zip", dir::root()?);
                    std::fs::copy(path, &dst).context("Failed to save skin pack")?;
                    Some(dst)
                } else {
                    None
                },
            ))
        })
    }
}

impl Scene for MainScene {
    fn enter(&mut self, tm: &mut TimeManager, target: Option<RenderTarget>) -> Result<()> {
        self.tab_start_time = f32::NEG_INFINITY;
        self.target = target;
        if let Some((_, st, _, true)) = &mut self.transit {
            *st = tm.now() as _;
        } else {
            show_message("欢迎回来");
        }
        if UPDATE_INFO.fetch_and(false, Ordering::SeqCst) {
            let Some((id, ..)) = self.transit else { unreachable!() };
            self.charts_local[id as usize].info = get_data().charts[id as usize].info.clone();
        }
        Ok(())
    }

    fn pause(&mut self, _tm: &mut TimeManager) -> Result<()> {
        save_data()?;
        self.cali_tm.pause();
        if let Some(handle) = &mut self.cali_handle {
            self.audio.pause(handle)?;
        }
        Ok(())
    }

    fn resume(&mut self, _tm: &mut TimeManager) -> Result<()> {
        self.cali_tm.resume();
        if let Some(handle) = &mut self.cali_handle {
            self.audio.resume(handle)?;
        }
        Ok(())
    }

    fn touch(&mut self, tm: &mut TimeManager, touch: Touch) -> Result<()> {
        if tm.now() as f32 <= self.tab_start_time + SWITCH_TIME || self.transit.is_some() {
            return Ok(());
        }
        if let Some(tab_id) = self.tab_buttons.iter_mut().position(|it| it.touch(&touch)) {
            if tab_id != self.tab_index {
                self.tab_from_index = self.tab_index;
                self.tab_index = tab_id;
                self.tab_start_time = tm.now() as f32;
                if self.tab_from_index == 2 {
                    save_data()?;
                }
                if tab_id == 1 && self.remote_first_time {
                    self.remote_first_time = false;
                    self.refresh_remote();
                }
                if tab_id == 3 {
                    self.cali_handle = Some(self.audio.play(
                        &self.cali_clip,
                        PlayParams {
                            loop_: true,
                            volume: 0.7,
                            ..Default::default()
                        },
                    )?);
                    self.cali_tm.reset();
                }
                if self.tab_from_index == 3 {
                    if let Some(handle) = &mut self.cali_handle {
                        self.audio.pause(handle)?;
                    }
                    self.cali_handle = None;
                }
                return Ok(());
            }
        }
        if self.import_button.touch(&touch) {
            request_file("chart");
        }
        let t = tm.now() as _;
        if self.tab_index == 0 && !self.scroll_local.touch(&touch, t) {
            if let Some(pos) = self.scroll_local.position(&touch) {
                let id = Self::get_touched(pos);
                let trigger = Self::trigger_grid(touch.phase, &mut self.choose_local, id);
                if trigger {
                    let id = id.unwrap();
                    if let Some(chart) = self.charts_local.get(id as usize) {
                        if chart.illustration_task.is_none() {
                            self.transit = Some((id, tm.now() as _, Rect::default(), false));
                            TRANSIT_ID.store(id, Ordering::SeqCst);
                        } else {
                            show_message("尚未加载完成");
                        }
                    }
                    return Ok(());
                }
            }
        } else {
            self.choose_local = None;
        }
        if self.tab_index == 1 && !self.scroll_remote.touch(&touch, t) {
            if let Some(pos) = self.scroll_remote.position(&touch) {
                let id = Self::get_touched(pos);
                let trigger = Self::trigger_grid(touch.phase, &mut self.choose_remote, id);
                if trigger {
                    let id = id.unwrap();
                    if id < self.charts_remote.len() as u32 {
                        let chart_id = self.charts_remote[id as usize].info.id.as_ref().unwrap();
                        dir::downloaded_charts()?;
                        let path = format!("download/{}", chart_id);
                        if get_data().charts.iter().any(|it| it.path == path) {
                            show_message("已经下载");
                            return Ok(());
                        }
                        if self.downloading.contains_key(chart_id) {
                            show_message("已经在下载队列中");
                            return Ok(());
                        }
                        show_message("正在下载");
                        let chart = &self.charts_remote[id as usize];
                        let url = chart.path.clone();
                        let chart = LocalChart {
                            info: chart.info.clone(),
                            path,
                        };
                        self.downloading.insert(
                            chart_id.clone(),
                            (
                                chart.info.name.clone(),
                                Task::new({
                                    let path = format!("{}/{}", dir::downloaded_charts()?, chart_id);
                                    async move {
                                        tokio::fs::write(path, reqwest::get(url).await?.bytes().await?).await?;
                                        Ok(chart)
                                    }
                                }),
                            ),
                        );
                        return Ok(());
                    }
                }
            }
        } else {
            self.choose_remote = None;
        }
        if self.tab_index == 2 && self.account_page.task.is_none() && get_data().me.is_some() && self.account_page.avatar_button.touch(&touch) {
            request_file("avatar");
        }
        if self.tab_index == 3 {
            for (id, button) in self.chal_buttons.iter_mut().enumerate() {
                if button.touch(&touch) {
                    use ChallengeModeColor::*;
                    get_data_mut().config.challenge_color = [White, Green, Blue, Red, Golden, Rainbow][id].clone();
                    save_data()?;
                }
            }
        }
        Ok(())
    }

    fn update(&mut self, tm: &mut TimeManager) -> Result<()> {
        let t = tm.now() as _;
        if self.scroll_remote.y_scroller.pulled {
            self.refresh_remote();
        }
        self.scroll_local.update(t);
        self.scroll_remote.update(t);
        let p = ((tm.now() as f32 - self.tab_start_time) / SWITCH_TIME).min(1.);
        if let Some(handle) = &self.cali_handle {
            let pos = self.audio.position(handle)?;
            let now = self.cali_tm.now();
            if now > 2. {
                self.cali_tm.seek_to(now - 2.);
                self.cali_tm.dont_wait();
            }
            let now = self.cali_tm.now();
            if now - pos >= -1. {
                self.cali_tm.update(pos);
            }
        }
        if p < 1. {
            let p = 1. - (1. - p).powi(3);
            self.tab_scroll
                .set_offset(f32::tween(&(self.tab_from_index as f32), &(self.tab_index as f32), p) * (1. - SIDE_PADDING) * 2., 0.);
        }
        let remove = self
            .downloading
            .iter_mut()
            .map(|(key, (_, task))| (key, task.ok()))
            .filter(|it| it.1)
            .map(|it| it.0.clone())
            .collect::<Vec<_>>();
        for id in remove {
            let mut task = self.downloading.remove(&id).unwrap();
            let res = task.1.take().unwrap();
            match res {
                Err(err) => {
                    show_error(err.context(format!("{} 下载失败", task.0)));
                }
                Ok(chart) => {
                    get_data_mut().charts.push(chart);
                    save_data()?;
                    self.charts_local = load_local(&self.tex);
                    show_message(format!("{} 下载完成", task.0));
                }
            }
        }
        if let Some(charts) = self.task_load.take() {
            self.loading_remote = false;
            match charts {
                Ok(charts) => {
                    show_message("加载完成");
                    self.charts_remote = charts;
                }
                Err(err) => {
                    self.remote_first_time = true;
                    show_error(err.context("加载失败"));
                }
            }
        }
        if let Some(result) = self.import_task.take() {
            match result {
                Err(err) => {
                    show_error(err.context("导入失败"));
                }
                Ok(chart) => {
                    get_data_mut().charts.push(chart);
                    save_data()?;
                    self.charts_local = load_local(&self.tex);
                    show_message("导入成功");
                }
            }
        }
        if let Some(future) = &mut self.load_skin_task {
            if let Some(result) = poll_future(future.as_mut()) {
                self.load_skin_task = None;
                match result {
                    Err(err) => {
                        show_error(err.context("加载皮肤失败"));
                    }
                    Ok((skin, dst)) => {
                        self.click_texture = skin.note_style.click.clone();
                        self.emitter = ParticleEmitter::new(&skin, get_data().config.note_scale)?;
                        self._skin = skin;
                        get_data_mut().config.skin_path = dst;
                        save_data()?;
                        show_message("加载皮肤成功");
                    }
                }
            }
        }
        if let Some((id, text)) = take_input() {
            if id == "edit_username" {
                if let Some(error) = validate_username(&text) {
                    show_message(error);
                } else {
                    let user = get_data().me.clone().unwrap();
                    self.account_page.start("更新名称", async move {
                        Client::update_user(json!({ "username": text })).await?;
                        Ok(Some(User { name: text, ..user }))
                    });
                }
            } else {
                return_input(id, text);
            }
        }
        if let Some((id, file)) = take_file() {
            match id.as_str() {
                "chart" => {
                    async fn import(from: String) -> Result<LocalChart> {
                        let file = NamedTempFile::new_in(dir::custom_charts()?)?.keep()?.1;
                        std::fs::copy(from, &file).context("Failed to save")?;
                        let mut fs = fs::fs_from_file(std::path::Path::new(&file))?;
                        let info = fs::load_info(fs.deref_mut()).await?;
                        Ok(LocalChart {
                            info: BriefChartInfo {
                                id: Option::None,
                                ..info.into()
                            },
                            path: format!("custom/{}", file.file_name().unwrap().to_str().unwrap()),
                        })
                    }
                    self.import_task = Task::new(import(file));
                }
                "avatar" => {
                    fn load(path: String, page: &mut AccountPage) -> Result<()> {
                        let image = image::load_from_memory(&std::fs::read(path).context("无法读取图片")?)
                            .context("无法加载图片")?
                            .resize_exact(512, 512, FilterType::CatmullRom);
                        let mut bytes: Vec<u8> = Vec::new();
                        image.write_to(&mut Cursor::new(&mut bytes), image::ImageOutputFormat::Png)?;
                        let old_avatar = get_data().me.as_ref().unwrap().avatar.clone();
                        let user = get_data().me.clone().unwrap();
                        page.start("上传头像", async move {
                            let file = Client::upload_file("avatar.png", &bytes).await.context("上传头像失败")?;
                            if let Some(old) = old_avatar {
                                Client::delete_file(&old.id).await.context("删除原头像失败")?;
                            }
                            Client::update_user(json!({ "avatar": {
                                "id": file.id,
                                "__type": "File"
                            } }))
                            .await
                            .context("更新头像失败")?;
                            UserManager::clear_cache(&user.id);
                            Ok(Some(User { avatar: Some(file), ..user }))
                        });
                        Ok(())
                    }
                    if let Err(err) = load(file, &mut self.account_page) {
                        show_error(err.context("导入头像失败"));
                    }
                }
                "skin" => {
                    self.load_skin_task = Some(Self::new_skin_task(Some(file)));
                }
                _ => return_file(id, file),
            }
        }
        if let Some(task) = self.account_page.task.as_mut() {
            if let Some(result) = task.take() {
                let desc = &self.account_page.task_desc;
                match result {
                    Err(err) => {
                        show_error(err.context(format!("{desc}失败")));
                    }
                    Ok(user) => {
                        if let Some(user) = user {
                            get_data_mut().me = Some(user);
                            save_data()?;
                        }
                        show_message(format!("{desc}成功"));
                        if desc == "注册" {
                            show_message("验证信息已发送到邮箱，请验证后登录");
                        }
                        self.account_page.register = false;
                    }
                }
                self.account_page.task = None;
            }
        }
        if let Some(tex) = UPDATE_TEXTURE.lock().unwrap().take() {
            let Some((id, ..)) = self.transit else { unreachable!() };
            self.charts_local[id as usize].illustration = tex;
        }
        Ok(())
    }

    fn render(&mut self, tm: &mut TimeManager, ui: &mut Ui) -> Result<()> {
        set_camera(&Camera2D {
            zoom: vec2(1., -screen_aspect()),
            render_target: self.target,
            ..Default::default()
        });
        clear_background(GRAY);
        ui.scope(|ui| self.ui(ui, tm.now() as f32));
        if let Some((id, st, rect, back)) = &mut self.transit {
            let t = tm.now() as f32;
            let p = ((t - *st) / TRANSIT_TIME).min(1.);
            let mut p = 1. - (1. - p).powi(4);
            if *back {
                p = 1. - p;
            }
            let rect = Rect::new(
                f32::tween(&rect.x, &-1., p),
                f32::tween(&rect.y, &-ui.top, p),
                f32::tween(&rect.w, &2., p),
                f32::tween(&rect.h, &(ui.top * 2.), p),
            );
            let path = {
                let mut path = Path::builder();
                let pad = CARD_PADDING * (1. - p);
                path.add_rounded_rectangle(
                    &lm::Box2D::new(lm::point(rect.x + pad, rect.y + pad), lm::point(rect.right() - pad, rect.bottom() - pad)),
                    &BorderRadii::new(0.01 * (1. - p)),
                    Winding::Positive,
                );
                path.build()
            };
            let chart = &self.charts_local[*id as usize];
            ui.fill_path(&path, (*chart.illustration, rect, ScaleType::Scale));
            ui.fill_path(&path, Color::new(0., 0., 0., 0.55));
            if *back && p <= 0. {
                if SHOULD_DELETE.fetch_and(false, Ordering::SeqCst) {
                    let err: Result<_> = (|| {
                        let Some((id, ..)) = self.transit else {unreachable!()};
                        let id = id as usize;
                        let path = format!("{}/{}", dir::charts()?, self.charts_local[id].path);
                        let path = std::path::Path::new(&path);
                        if path.is_file() {
                            std::fs::remove_file(path)?;
                        } else {
                            std::fs::remove_dir_all(path)?;
                        }
                        get_data_mut().charts.remove(id);
                        save_data()?;
                        self.charts_local.remove(id);
                        Ok(())
                    })();
                    if let Err(err) = err {
                        show_error(err.context("删除失败"));
                    } else {
                        show_message("删除成功");
                    }
                }
                self.transit = None;
            } else if !*back && p >= 1. {
                self.next_scene = Some(NextScene::Overlay(Box::new(SongScene::new(
                    ChartItem {
                        info: chart.info.clone(),
                        path: chart.path.clone(),
                        illustration: chart.illustration.clone(),
                        illustration_task: None,
                    },
                    chart.illustration.clone(),
                    self.icon_edit.clone(),
                    self.icon_back.clone(),
                    self.icon_play.clone(),
                    TrashBin::new(self.icon_delete.clone(), self.icon_question.clone()),
                ))));
                *back = true;
            }
        }
        Ok(())
    }

    fn next_scene(&mut self, _tm: &mut TimeManager) -> NextScene {
        self.next_scene.take().unwrap_or_default()
    }
}
