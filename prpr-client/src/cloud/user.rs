use super::{Client, Images, User};
use anyhow::Result;
use image::{DynamicImage, GenericImage, Rgba};
use macroquad::prelude::warn;
use once_cell::sync::Lazy;
use prpr::{ext::SafeTexture, task::Task};
use std::{collections::HashMap, sync::Mutex};

static TASKS: Lazy<Mutex<HashMap<String, Task<Result<DynamicImage>>>>> = Lazy::new(Mutex::default);
static RESULTS: Lazy<Mutex<HashMap<String, (String, Option<SafeTexture>)>>> = Lazy::new(Mutex::default);

pub struct UserManager;

impl UserManager {
    pub fn clear_cache(user_id: &str) {
        RESULTS.lock().unwrap().remove(user_id);
    }

    pub fn cache(user: User) {
        let mut tasks = TASKS.lock().unwrap();
        if tasks.contains_key(&user.id) {
            return;
        }
        tasks.insert(
            user.id.clone(),
            Task::new(async move {
                let image = if let Some(avatar) = user.avatar {
                    Images::load_lc(&avatar).await?
                } else {
                    let mut image = image::DynamicImage::new_rgba8(1, 1);
                    image.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
                    image
                };
                Ok(image)
            }),
        );
    }

    pub fn request(user_id: &str) {
        let mut tasks = TASKS.lock().unwrap();
        if tasks.contains_key(user_id) {
            return;
        }
        let id = user_id.to_owned();
        tasks.insert(
            id.clone(),
            Task::new(async move {
                let user = Client::fetch::<User>(id.clone()).await?;
                RESULTS.lock().unwrap().insert(id, (user.name.clone(), None));
                let image = if let Some(avatar) = user.avatar {
                    Images::load_lc(&avatar).await?
                } else {
                    let mut image = image::DynamicImage::new_rgba8(1, 1);
                    image.put_pixel(0, 0, Rgba([0, 0, 0, 255]));
                    image
                };
                Ok(image)
            }),
        );
    }

    pub fn get_name(user_id: &str) -> Option<String> {
        let names = RESULTS.lock().unwrap();
        if let Some((name, _)) = names.get(user_id) {
            return Some(name.clone());
        }
        None
    }

    pub fn get_avatar(user_id: &str) -> Option<SafeTexture> {
        let mut guard = TASKS.lock().unwrap();
        if let Some(task) = guard.get_mut(user_id) {
            if let Some(result) = task.take() {
                match result {
                    Err(err) => {
                        warn!("Failed to fetch user info: {:?}", err);
                        guard.remove(user_id);
                    }
                    Ok(image) => {
                        RESULTS.lock().unwrap().get_mut(user_id).unwrap().1 = Some(image.into());
                    }
                }
            }
        } else {
            drop(guard);
        }
        RESULTS.lock().unwrap().get(user_id).and_then(|it| it.1.clone())
    }
}
