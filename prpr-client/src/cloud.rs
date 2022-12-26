mod file;
use std::marker::PhantomData;

use file::upload_qiniu;

mod structs;
pub use structs::*;

mod user;
pub use user::UserManager;

use crate::get_data;
use anyhow::{bail, Context, Result};
use reqwest::{header, Method, RequestBuilder};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{json, Value};

async fn recv_lc(request: RequestBuilder) -> Result<String> {
    #[derive(Deserialize)]
    struct ErrorMsg {
        code: i32,
        error: String,
    }
    let resp = request.send().await.context("Failed to send request")?;
    let status = resp.status();
    let text = resp.text().await.context("Failed to receive text")?;
    if !status.is_success() {
        let error: ErrorMsg = serde_json::from_str(&text).context("Failed to parse error message")?;
        bail!("Operation not success. Error code {}. Error: {}", error.code, error.error);
    }
    Ok(text)
}

async fn parse_lc<T: LCObject>(request: RequestBuilder) -> Result<T> {
    serde_json::from_str(&recv_lc(request).await?).context("Failed to parse content")
}

async fn parse_lc_many<T: LCObject>(request: RequestBuilder) -> Result<Vec<T>> {
    let mut json: serde_json::Value = serde_json::from_str(&recv_lc(request).await?).context("Failed to parse content")?;
    let mut results = json["results"].take();
    std::mem::take(results.as_array_mut().unwrap())
        .into_iter()
        .map(|it| Ok(serde_json::from_value(it)?))
        .collect::<Result<_>>()
}

pub trait LCObject: DeserializeOwned {
    const CLASS_NAME: &'static str;
}

const API_URL: &str = "https://uxjq2roe.lc-cn-n1-shared.com/1.1";
const API_ID: &str = "uxjq2ROe26ucGlFXIbWYOhEW-gzGzoHsz";
const API_KEY: &str = "LW6yy6lkSFfXDqZo0442oFjT";

pub trait RequestExt {
    fn with_session(self) -> Self;
}

impl RequestExt for RequestBuilder {
    fn with_session(self) -> Self {
        self.header("X-LC-Session", get_data().me.as_ref().unwrap().session_token.as_ref().unwrap())
    }
}

#[derive(Deserialize)]
struct UploadToken {
    #[serde(rename = "objectId")]
    object_id: String,
    upload_url: String,
    key: String,
    token: String,
    url: String,
    provider: String,
    bucket: String,
}

#[must_use = "QueryBuilder does nothing until you 'send' it"]
#[derive(Serialize)]
pub struct QueryBuilder<T: LCObject> {
    #[serde(rename = "where")]
    where_: Option<String>,
    order: Option<String>,
    #[serde(skip)]
    phantom: PhantomData<T>,
}

impl<T: LCObject> QueryBuilder<T> {
    pub fn with_where(mut self, clause: Value) -> Self {
        self.where_ = Some(clause.to_string());
        self
    }

    pub fn order(mut self, order: impl Into<String>) -> Self {
        self.order = Some(order.into());
        self
    }

    pub async fn send(self) -> Result<Vec<T>> {
        parse_lc_many(Client::get(format!("/classes/{}", T::CLASS_NAME)).form(&self)).await
    }
}

pub struct Client;

impl Client {
    fn get(path: impl AsRef<str>) -> RequestBuilder {
        Self::request(Method::GET, path)
    }

    fn post(path: impl AsRef<str>, data: Value) -> RequestBuilder {
        Self::request(Method::POST, path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(data.to_string())
    }

    fn put(path: impl AsRef<str>, data: Value) -> RequestBuilder {
        Self::request(Method::PUT, path)
            .header(header::CONTENT_TYPE, "application/json")
            .body(data.to_string())
    }

    fn request(method: Method, path: impl AsRef<str>) -> RequestBuilder {
        reqwest::Client::new()
            .request(method, API_URL.to_string() + path.as_ref())
            .header("X-LC-Id", API_ID)
            .header("X-LC-Key", API_KEY)
    }

    pub async fn fetch<T: LCObject>(ptr: impl Into<Pointer>) -> Result<T> {
        parse_lc(Self::get(format!("/classes/{}/{}", T::CLASS_NAME, ptr.into().id))).await
    }

    pub async fn create<T: LCObject + Serialize>(value: T) -> Result<T> {
        recv_lc(Self::post(format!("/classes/{}", T::CLASS_NAME), serde_json::to_value(&value)?)).await?;
        Ok(value)
    }

    pub fn query<T: LCObject>() -> QueryBuilder<T> {
        QueryBuilder {
            where_: None,
            order: None,
            phantom: PhantomData::default(),
        }
    }

    pub async fn register(email: &str, username: &str, password: &str) -> Result<()> {
        recv_lc(Self::post(
            "/users",
            json!({
                "email": email,
                "username": username,
                "password": password,
            }),
        ))
        .await?;
        Ok(())
    }

    pub async fn login(username: &str, password: &str) -> Result<User> {
        parse_lc(Self::post(
            "/login",
            json!({
                "username": username,
                "password": password,
            }),
        ))
        .await
    }

    pub async fn update_user(patch: Value) -> Result<()> {
        recv_lc(Self::put(format!("/users/{}", get_data().me.as_ref().unwrap().id), patch).with_session()).await?;
        Ok(())
    }

    pub async fn get_me() -> Result<User> {
        parse_lc(Self::get("/users/me").with_session()).await
    }

    pub async fn upload_file(name: &str, data: &[u8]) -> Result<LCFile> {
        let checksum = format!("{:x}", md5::compute(data));
        let id = get_data().me.as_ref().unwrap().id.clone();
        let mut token: UploadToken = serde_json::from_str(
            &recv_lc(Self::post(
                "/fileTokens",
                json!({
                    "name": name,
                    "__type": "File",
                    "ACL": {
                        id: {
                            "read": true,
                            "write": true,
                        },
                        "*": {
                            "read": true
                        }
                    },
                    "metaData": {
                        "size": data.len(),
                        "_checksum": checksum,
                    }
                }),
            ))
            .await?,
        )?;
        if token.provider != "qiniu" {
            bail!("Unsupported prvider: {}", token.provider);
        }
        let file = LCFile::new(std::mem::take(&mut token.object_id), std::mem::take(&mut token.url));
        let token_s = token.token.clone();
        upload_qiniu(token, data).await?;
        let _ = recv_lc(Self::post(
            "/fileCallback",
            json!({
                "result": true,
                "token": token_s,
            }),
        ))
        .await;
        Ok(file)
    }

    pub async fn delete_file(id: &str) -> Result<()> {
        recv_lc(Self::request(Method::DELETE, format!("/files/{id}")).with_session()).await?;
        Ok(())
    }
}
