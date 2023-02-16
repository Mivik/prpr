mod chart;
pub use chart::*;

mod record;
pub use record::*;

mod song;
pub use song::*;

mod user;
pub use user::*;

use crate::images::{THUMBNAIL_HEIGHT, THUMBNAIL_WIDTH};

use super::Client;
use anyhow::Result;
use bytes::Bytes;
use futures_util::Stream;
use http_cache_reqwest::{CACacheManager, Cache, CacheMode, HttpCache};
use image::DynamicImage;
use lru::LruCache;
use macroquad::prelude::info;
use once_cell::sync::Lazy;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use serde::{de::DeserializeOwned, Deserialize, Serialize, Serializer};
use std::{
    any::Any,
    collections::HashMap,
    sync::{Arc, Mutex},
};

pub(crate) type ObjectMap<T> = LruCache<u64, Arc<T>>;
static CACHES: Lazy<Mutex<HashMap<&'static str, Arc<Mutex<Box<dyn Any + Send + Sync>>>>>> = Lazy::new(Mutex::default);

pub(crate) fn obtain_map_cache<T: PZObject + 'static>() -> Arc<Mutex<Box<dyn Any + Send + Sync>>> {
    let mut caches = CACHES.lock().unwrap();
    Arc::clone(
        caches
            .entry(T::QUERY_PATH)
            .or_insert_with(|| Arc::new(Mutex::new(Box::new(ObjectMap::<T>::new(64.try_into().unwrap()))))),
    )
}

pub trait PZObject: Clone + DeserializeOwned + Send + Sync {
    const QUERY_PATH: &'static str;

    fn id(&self) -> u64;
}

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "String")]
pub struct MusicPosition {
    pub seconds: u32,
}
impl TryFrom<String> for MusicPosition {
    type Error = &'static str;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let seconds = || -> Option<u32> {
            let mut it = value.splitn(3, ':');
            let mut res = it.next()?.parse::<u32>().ok()?;
            res = res * 60 + it.next()?.parse::<u32>().ok()?;
            res = res * 60 + it.next()?.parse::<u32>().ok()?;
            Some(res)
        }()
        .ok_or("illegal position")?;
        Ok(MusicPosition { seconds })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(try_from = "u8")]
#[repr(u8)]
pub enum LevelType {
    EZ = 0,
    HD,
    IN,
    AT,
    SP,
}
impl TryFrom<u8> for LevelType {
    type Error = String;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        use LevelType::*;
        Ok(match value {
            0 => EZ,
            1 => HD,
            2 => IN,
            3 => AT,
            4 => SP,
            x => {
                return Err(format!("illegal level type: {x}"));
            }
        })
    }
}

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum RawPZPointer<T> {
    Id(u64),
    Concrete(T),
}

#[derive(Debug, Deserialize)]
#[serde(bound = "T: PZObject + 'static")]
#[serde(from = "RawPZPointer<T>")]
pub enum PZPointer<T> {
    Id(u64),
    Concrete(Arc<T>),
}
impl<T: PZObject> Clone for PZPointer<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Id(id) => Self::Id(*id),
            Self::Concrete(value) => Self::Concrete(Arc::clone(value)),
        }
    }
}
impl<T: PZObject> From<u64> for PZPointer<T> {
    fn from(value: u64) -> Self {
        Self::Id(value)
    }
}
impl<T: PZObject + 'static> From<RawPZPointer<T>> for PZPointer<T> {
    fn from(value: RawPZPointer<T>) -> Self {
        match value {
            RawPZPointer::Id(id) => Self::Id(id),
            RawPZPointer::Concrete(value) => {
                let map = obtain_map_cache::<T>();
                let mut guard = map.lock().unwrap();
                let Some(actual_map) = guard.downcast_mut::<ObjectMap::<T>>() else {
                    unreachable!()
                };
                let id = value.id();
                let value = Arc::new(value);
                actual_map.put(id, Arc::clone(&value));
                Self::Concrete(value)
            }
        }
    }
}

impl<T: PZObject + 'static> PZPointer<T> {
    pub fn id(&self) -> u64 {
        match self {
            Self::Id(id) => *id,
            Self::Concrete(value) => value.id(),
        }
    }

    #[inline]
    pub async fn fetch(&self) -> Result<Arc<T>> {
        Client::fetch(self.id()).await.map(|it| it.unwrap())
    }

    pub async fn load(&self) -> Result<Arc<T>> {
        match self {
            Self::Id(id) => {
                // sync locks can not be held accross await point
                {
                    let map = obtain_map_cache::<T>();
                    let mut guard = map.lock().unwrap();
                    let Some(actual_map) = guard.downcast_mut::<ObjectMap::<T>>() else { unreachable!() };
                    if let Some(value) = actual_map.get(id) {
                        return Ok(Arc::clone(value));
                    }
                    drop(guard);
                    drop(map);
                }
                self.fetch().await
            }
            Self::Concrete(value) => Ok(Arc::clone(value)),
        }
    }

    pub fn as_ref(&self) -> Option<&T> {
        match self {
            Self::Concrete(value) => Some(value),
            _ => None,
        }
    }

    pub fn value(self) -> Option<Arc<T>> {
        match self {
            Self::Concrete(value) => Some(value),
            _ => None,
        }
    }
}
impl<T: PZObject + 'static> Serialize for PZPointer<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_u64(self.id())
    }
}

static CACHE_CLIENT: Lazy<ClientWithMiddleware> = Lazy::new(|| {
    ClientBuilder::new(reqwest::Client::new())
        .with(Cache(HttpCache {
            mode: CacheMode::Default,
            manager: CACacheManager::default(),
            options: None,
        }))
        .build()
});

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PZFile {
    pub url: String,
}
impl PZFile {
    pub async fn fetch(&self) -> Result<Bytes> {
        Ok(CACHE_CLIENT.get(&self.url).send().await?.bytes().await?)
    }

    pub async fn fetch_stream(&self) -> Result<impl Stream<Item = reqwest::Result<Bytes>>> {
        Ok(CACHE_CLIENT.get(&self.url).send().await?.bytes_stream())
    }

    pub async fn load_image(&self) -> Result<DynamicImage> {
        Ok(image::load_from_memory(&self.fetch().await?)?)
    }

    pub async fn load_thumbnail(&self) -> Result<DynamicImage> {
        if self.url.starts_with("https://res.phi.zone/") {
            if let Some(pre) = self.url.strip_suffix(".webp") {
                return Ok(PZFile {
                    url: format!("{pre}.comp.webp"),
                }
                .load_image()
                .await?);
            }
        }
        if self.url.starts_with("http://phizone.mivik.cn/") {
            return Ok(PZFile {
                url: format!("{}?imageView/0/w/{THUMBNAIL_WIDTH}/h/{THUMBNAIL_HEIGHT}", self.url),
            }
            .load_image()
            .await?);
        }
        self.load_image().await
    }
}
