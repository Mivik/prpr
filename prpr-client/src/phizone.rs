mod object;
use std::{borrow::Cow, collections::HashMap, marker::PhantomData, sync::Arc};

pub use object::*;

use anyhow::{anyhow, bail, Context, Result};
use object::PZObject;
use once_cell::sync::Lazy;
use reqwest::{header, Method, RequestBuilder, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio::sync::RwLock;

const CLIENT_ID: &str = env!("CLIENT_ID");
const CLIENT_SECRET: &str = env!("CLIENT_SECRET");

static CLIENT: Lazy<RwLock<reqwest::Client>> = Lazy::new(|| RwLock::new(reqwest::Client::new()));

pub struct Client;

const API_URL: &str = "http://localhost:3000";

async fn recv_raw(request: RequestBuilder) -> Result<Response> {
    let response = request.send().await?;
    if !response.status().is_success() {
        let text = response.text().await.context("failed to receive text")?;
        if let Ok(what) = serde_json::from_str::<serde_json::Value>(&text) {
            bail!("request failed: {}", what["detail"].as_str().unwrap());
        } else {
            bail!("request failed: {text}");
        }
    }
    Ok(response)
}

impl Client {
    #[inline]
    pub async fn get(path: impl AsRef<str>) -> RequestBuilder {
        Self::request(Method::GET, path).await
    }

    #[inline]
    pub async fn post<T: Serialize>(path: impl AsRef<str>, data: &T) -> RequestBuilder {
        Self::request(Method::POST, path).await.json(data)
    }

    #[inline]
    pub async fn put<T: Serialize>(path: impl AsRef<str>, data: &T) -> RequestBuilder {
        Self::request(Method::PUT, path).await.json(data)
    }

    pub async fn request(method: Method, path: impl AsRef<str>) -> RequestBuilder {
        CLIENT.read().await.request(method, API_URL.to_string() + path.as_ref())
    }

    pub async fn load<T: PZObject + 'static>(id: u64) -> Result<Option<Arc<T>>> {
        {
            let map = obtain_map_cache::<T>();
            let mut guard = map.lock().unwrap();
            let Some(actual_map) = guard.downcast_mut::<ObjectMap::<T>>() else { unreachable!() };
            if let Some(value) = actual_map.get(&id) {
                return Ok(Some(Arc::clone(value)));
            }
            drop(guard);
            drop(map);
        }
        Self::fetch(id).await
    }

    pub async fn fetch<T: PZObject + 'static>(id: u64) -> Result<Option<Arc<T>>> {
        let value = Arc::new(Client::fetch_inner::<T>(id).await?.ok_or_else(|| anyhow!("entry not found"))?);
        let map = obtain_map_cache::<T>();
        let mut guard = map.lock().unwrap();
        let Some(actual_map) = guard.downcast_mut::<ObjectMap::<T>>() else {
            unreachable!()
        };
        Ok(Some(Arc::clone(actual_map.get_or_insert(id, || value))))
    }

    async fn fetch_inner<T: PZObject>(id: u64) -> Result<Option<T>> {
        Ok(recv_raw(
            Self::get(format!("/{}/{id}/", T::QUERY_PATH))
                .await
                .query(&[("query_owner", "1"), ("query_records", "1")]),
        )
        .await?
        .json()
        .await?)
    }

    pub fn query<T: PZObject>() -> QueryBuilder<T> {
        QueryBuilder {
            queries: HashMap::new(),
            page: None,
            _phantom: PhantomData::default(),
        }
    }

    pub async fn register(email: &str, username: &str, password: &str) -> Result<()> {
        recv_raw(
            Self::post(
                "/register/",
                &json!({
                    "email": email,
                    "username": username,
                    "password": password,
                    "language": "zh-Hans", // TODO
                }),
            )
            .await,
        )
        .await?;
        Ok(())
    }

    pub async fn login(email: &str, password: &str) -> Result<()> {
        #[derive(Deserialize)]
        struct Resp {
            access_token: String,
            refresh_token: String,
        }
        let resp: Resp = recv_raw(
            Self::post(
                "/auth/token/",
                &json!({
                    "client_id": CLIENT_ID,
                    "client_secret": CLIENT_SECRET,
                    "grant_type": "password",
                    "username": email,
                    "password": password,
                }),
            )
            .await,
        )
        .await?
        .json()
        .await?;

        let mut headers = header::HeaderMap::new();
        let mut auth_value = header::HeaderValue::from_str(&format!("Bearer {}", resp.access_token))?;
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);
        *CLIENT.write().await = reqwest::ClientBuilder::new().default_headers(headers).build()?;
        Ok(())
    }

    pub async fn get_me() -> Result<PZUser> {
        Ok(Self::get("/user_detail/").await.send().await?.json().await?)
    }
}

#[must_use]
pub struct QueryBuilder<T> {
    queries: HashMap<Cow<'static, str>, Cow<'static, str>>,
    page: Option<u64>,
    _phantom: PhantomData<T>,
}

impl<T: PZObject> QueryBuilder<T> {
    pub fn query(mut self, key: impl Into<Cow<'static, str>>, value: impl Into<Cow<'static, str>>) -> Self {
        self.queries.insert(key.into(), value.into());
        self
    }

    #[inline]
    pub fn order(mut self, order: impl Into<Cow<'static, str>>) -> Self {
        self.query("order", order)
    }

    pub fn flag(mut self, flag: impl Into<Cow<'static, str>>) -> Self {
        self.queries.insert(flag.into(), "1".into());
        self
    }

    pub fn page(mut self, page: u64) -> Self {
        self.page = Some(page);
        self
    }

    pub async fn send(mut self) -> Result<(Vec<T>, u64)> {
        if let Some(page) = self.page {
            self.queries.insert("page".into(), (page + 1).to_string().into());
            #[derive(Deserialize)]
            struct PagedResult<T> {
                count: u64,
                results: Vec<T>,
            }
            let res: PagedResult<T> = recv_raw(Client::get(format!("/{}/", T::QUERY_PATH)).await.query(&self.queries))
                .await?
                .json()
                .await?;
            Ok((res.results, res.count))
        } else {
            self.queries.insert("pagination".into(), "0".into());
            let res: Vec<T> = recv_raw(Client::get(format!("/{}/", T::QUERY_PATH)).await.query(&self.queries))
                .await?
                .json()
                .await?;
            let count = res.len() as u64;
            Ok((res, count))
        }
    }
}
