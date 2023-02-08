prpr::tl_file!("account");

use super::{Page, SharedState};
use crate::{
    cloud::{Client, User, UserManager},
    get_data, get_data_mut, save_data, Rect, Ui,
};
use anyhow::{Context, Result};
use image::imageops::FilterType;
use macroquad::prelude::Touch;
use once_cell::sync::Lazy;
use prpr::{
    scene::{request_file, request_input, return_file, return_input, show_error, show_message, take_file, take_input},
    task::Task,
    ui::RectButton,
};
use regex::Regex;
use serde_json::json;
use std::{borrow::Cow, future::Future, io::Cursor};

fn validate_username(username: &str) -> Option<Cow<'static, str>> {
    if !(4..=20).contains(&username.len()) {
        return Some(tl!("name-length-req"));
    }
    if username.chars().any(|it| it != '_' && it != '-' && !it.is_alphanumeric()) {
        return Some(tl!("name-has-illegal-char"));
    }
    None
}

pub struct AccountPage {
    register: bool,
    task: Option<Task<Result<Option<User>>>>,
    task_name: String,
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
            task_name: if logged_in { "update".to_owned() } else { String::new() },
            email_input: String::new(),
            username_input: String::new(),
            password_input: String::new(),
            avatar_button: RectButton::new(),
        }
    }

    pub fn start(&mut self, desc: impl Into<String>, future: impl Future<Output = Result<Option<User>>> + Send + 'static) {
        self.task_name = desc.into();
        self.task = Some(Task::new(future));
    }
}

impl Page for AccountPage {
    fn label(&self) -> Cow<'static, str> {
        tl!("label")
    }

    fn update(&mut self, _focus: bool, _state: &mut SharedState) -> Result<()> {
        if let Some(task) = self.task.as_mut() {
            if let Some(result) = task.take() {
                let action = self.task_name.as_str();
                match result {
                    Err(err) => show_error(err.context(tl!("action-failed", "action" => action))),
                    Ok(user) => {
                        if let Some(user) = user {
                            UserManager::request(&user.id);
                            get_data_mut().me = Some(user);
                            save_data()?;
                        }
                        show_message(tl!("action-success", "action" => action)).ok().duration(1.);
                        if action == "register" {
                            show_message(tl!("email-sent"));
                        }
                        self.register = false;
                    }
                }
                self.task = None;
            }
        }

        if let Some((id, text)) = take_input() {
            if id == "edit_username" {
                if let Some(error) = validate_username(&text) {
                    show_message(error);
                } else {
                    let user = get_data().me.clone().unwrap();
                    self.start("edit-name", async move {
                        Client::update_user(json!({ "username": text })).await?;
                        Ok(Some(User { name: text, ..user }))
                    });
                }
            } else {
                return_input(id, text);
            }
        }
        if let Some((id, file)) = take_file() {
            if id == "avatar" {
                let mut load = |path: String| -> Result<()> {
                    let image = image::load_from_memory(&std::fs::read(path).with_context(|| tl!("picture-read-failed"))?)
                        .with_context(|| tl!("picture-load-failed"))?
                        .resize_exact(512, 512, FilterType::CatmullRom);
                    let mut bytes: Vec<u8> = Vec::new();
                    image.write_to(&mut Cursor::new(&mut bytes), image::ImageOutputFormat::Png)?;
                    let old_avatar = get_data().me.as_ref().unwrap().avatar.clone();
                    let user = get_data().me.clone().unwrap();
                    self.start("set-avatar", async move {
                        let file = Client::upload_file("avatar.png", &bytes)
                            .await
                            .with_context(|| tl!("avatar-upload-failed"))?;
                        if let Some(old) = old_avatar {
                            Client::delete_file(&old.id).await.with_context(|| tl!("avatar-delete-old-failed"))?;
                        }
                        Client::update_user(json!({ "avatar": {
                                "id": file.id,
                                "__type": "File"
                            } }))
                        .await
                        .with_context(|| tl!("avatar-update-failed"))?;
                        UserManager::clear_cache(&user.id);
                        Ok(Some(User { avatar: Some(file), ..user }))
                    });
                    Ok(())
                };
                if let Err(err) = load(file) {
                    show_error(err.context(tl!("avatar-import-failed")));
                }
            } else {
                return_file(id, file);
            }
        }
        Ok(())
    }

    fn touch(&mut self, touch: &Touch, _state: &mut SharedState) -> Result<bool> {
        if self.task.is_none() && get_data().me.is_some() && self.avatar_button.touch(touch) {
            request_file("avatar");
            return Ok(true);
        }
        Ok(false)
    }

    fn render(&mut self, ui: &mut Ui, _state: &mut SharedState) -> Result<()> {
        ui.dx(0.02);
        let r = Rect::new(0., 0., 0.22, 0.22);
        self.avatar_button.set(ui, r);
        if let Some(avatar) = get_data().me.as_ref().and_then(|it| UserManager::get_avatar(&it.id)) {
            let ct = r.center();
            ui.fill_circle(ct.x, ct.y, r.w / 2., (*avatar, r));
        }
        ui.text(
            get_data()
                .me
                .as_ref()
                .map(|it| Cow::Borrowed(it.name.as_str()))
                .unwrap_or_else(|| tl!("not-logged-in")),
        )
        .pos(r.right() + 0.02, r.center().y)
        .anchor(0., 0.5)
        .size(0.8)
        .draw();
        ui.dy(r.h + 0.03);
        if get_data().me.is_none() {
            ui.dx(0.15);
            if self.register {
                let r = ui.input(tl!("email"), &mut self.email_input, ());
                ui.dy(r.h + 0.02);
            }
            let r = ui.input(tl!("username"), &mut self.username_input, ());
            ui.dy(r.h + 0.02);
            let r = ui.input(tl!("password"), &mut self.password_input, true);
            ui.dy(r.h + 0.02);
            let labels = if self.register {
                [tl!("back"), if self.task.is_none() { tl!("register") } else { tl!("registering") }]
            } else {
                [tl!("register"), if self.task.is_none() { tl!("login") } else { tl!("logging-in") }]
            };
            let cx = r.right() / 2.;
            let mut r = Rect::new(0., 0., cx - 0.01, r.h);
            if ui.button("left", r, labels[0].as_ref()) {
                self.register ^= true;
            }
            r.x = cx + 0.01;
            if ui.button("right", r, labels[1].as_ref()) {
                let mut login = || -> Option<Cow<'static, str>> {
                    let username = self.username_input.clone();
                    let password = self.password_input.clone();
                    if let Some(error) = validate_username(&username) {
                        return Some(error);
                    }
                    if !(6..=26).contains(&password.len()) {
                        return Some(tl!("pwd-length-req"));
                    }
                    if self.register {
                        let email = self.email_input.clone();
                        static EMAIL_REGEX: Lazy<Regex> = Lazy::new(|| Regex::new(r"^[\w\-\.]+@([\w\-]+\.)+[\w\-]{2,4}$").unwrap());
                        if !EMAIL_REGEX.is_match(&email) {
                            return Some(tl!("illegal-email"));
                        }
                        self.start("register", async move {
                            Client::register(&email, &username, &password).await?;
                            Ok(None)
                        });
                    } else {
                        self.start("login", async move {
                            let user = Client::login(&username, &password).await?;
                            Ok(Some(user))
                        });
                    }
                    None
                };
                if let Some(err) = login() {
                    show_message(err);
                }
            }
        } else {
            let cx = 0.2;
            let mut r = Rect::new(0., 0., cx - 0.01, 0.06);
            if ui.button("logout", r, tl!("logout")) && self.task.is_none() {
                get_data_mut().me = None;
                let _ = save_data();
                show_message(tl!("logged-out"));
            }
            r.x = cx + 0.01;
            if ui.button("edit_name", r, tl!("edit-name")) && self.task.is_none() {
                request_input("edit_username", &get_data().me.as_ref().unwrap().name);
            }
        }
        Ok(())
    }
}
