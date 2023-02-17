use crate::phizone::Client;

use super::{PZFile, PZObject, Ptr};
use anyhow::Result;
use chrono::{DateTime, Utc};
use image::DynamicImage;
use macroquad::prelude::warn;
use once_cell::sync::Lazy;
use prpr::{ext::SafeTexture, task::Task};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tokio::sync::Mutex;

#[derive(Debug, Serialize, Deserialize, Copy, Clone)]
#[repr(u8)]
#[serde(rename_all = "lowercase")]
pub enum PZUserRole {
    Banned = 0,
    Member,
    Qualified,
    Volunteer,
    Admin,
}

impl PZUserRole {
    pub fn priority(&self) -> u8 {
        *self as u8
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PZUser {
    pub id: u64,
    #[serde(rename = "username")]
    pub name: String,
    pub avatar: PZFile,
    pub gender: u8,
    pub bio: Option<String>,
    #[serde(rename = "type")]
    pub role: PZUserRole,

    #[serde(rename = "following")]
    pub num_following: u32,
    #[serde(rename = "fans")]
    pub num_follower: u32,

    pub tag: Option<String>,
    pub exp: u32,
    pub rks: f32,

    pub language: String,
    #[serde(rename = "is_active")]
    pub active: bool,

    pub last_login: DateTime<Utc>,
    pub date_joined: DateTime<Utc>,
    pub date_of_birth: Option<String>,

    pub extra: Option<PZUserExtra>,
}
impl PZObject for PZUser {
    const QUERY_PATH: &'static str = "users";

    fn id(&self) -> u64 {
        self.id
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PZUserExtra {}

static TASKS: Lazy<Mutex<HashMap<u64, Task<Result<DynamicImage>>>>> = Lazy::new(Mutex::default);
static RESULTS: Lazy<Mutex<HashMap<u64, (String, Option<SafeTexture>)>>> = Lazy::new(Mutex::default);

pub struct UserManager;

impl UserManager {
    pub fn clear_cache(id: u64) {
        RESULTS.blocking_lock().remove(&id);
    }

    pub fn request(id: u64) {
        let mut tasks = TASKS.blocking_lock();
        if tasks.contains_key(&id) {
            return;
        }
        tasks.insert(
            id,
            Task::new(async move {
                let user: Arc<PZUser> = Client::load(id).await?.unwrap();
                RESULTS.lock().await.insert(id, (user.name.clone(), None));
                let image = user.avatar.fetch().await?;
                let image = image::load_from_memory(&image)?;
                Ok(image)
            }),
        );
    }

    pub fn get_name(id: u64) -> Option<String> {
        let names = RESULTS.blocking_lock();
        if let Some((name, _)) = names.get(&id) {
            return Some(name.clone());
        }
        None
    }

    pub fn get_avatar(id: u64) -> Option<SafeTexture> {
        let mut guard = TASKS.blocking_lock();
        if let Some(task) = guard.get_mut(&id) {
            if let Some(result) = task.take() {
                match result {
                    Err(err) => {
                        warn!("Failed to fetch user info: {:?}", err);
                        guard.remove(&id);
                    }
                    Ok(image) => {
                        RESULTS.blocking_lock().get_mut(&id).unwrap().1 = Some(image.into());
                    }
                }
            }
        } else {
            drop(guard);
        }
        RESULTS.blocking_lock().get(&id).and_then(|it| it.1.clone())
    }
}
