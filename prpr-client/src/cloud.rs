mod structs;
pub use structs::*;

use anyhow::{bail, Context, Result};
use reqwest::{Method, RequestBuilder};
use serde::{de::DeserializeOwned, Deserialize};

async fn lc_recv(request: RequestBuilder) -> Result<String> {
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
    Ok(serde_json::from_str(&lc_recv(request).await?).context("Failed to parse content")?)
}

async fn parse_lc_many<T: LCObject>(request: RequestBuilder) -> Result<Vec<T>> {
    let mut json: serde_json::Value = serde_json::from_str(&lc_recv(request).await?).context("Failed to parse content")?;
    let mut results = json["results"].take();
    Ok(std::mem::take(results.as_array_mut().unwrap())
        .into_iter()
        .map(|it| Ok(serde_json::from_value(it)?))
        .collect::<Result<_>>()?)
}

pub trait LCObject: DeserializeOwned {
    const CLASS_NAME: &'static str;
}

pub struct Client {
    http: reqwest::Client,

    api_url: String,
    app_id: String,
    app_key: String,
}

impl Client {
    pub fn new(api_url: impl Into<String>, app_id: impl Into<String>, app_key: impl Into<String>) -> Self {
        Self {
            http: reqwest::Client::new(),

            api_url: api_url.into(),
            app_id: app_id.into(),
            app_key: app_key.into(),
        }
    }

    fn get(&self, path: impl AsRef<str>) -> RequestBuilder {
        self.request(Method::GET, path)
    }

    fn post(&self, path: impl AsRef<str>) -> RequestBuilder {
        self.request(Method::POST, path)
    }

    fn request(&self, method: Method, path: impl AsRef<str>) -> RequestBuilder {
        let path = path.as_ref();
        let mut url = self.api_url.clone();
        url.reserve_exact(path.len());
        url.push_str(path.as_ref());
        self.http
            .request(method, url)
            .header("X-LC-Id", &self.app_id)
            .header("X-LC-Key", &self.app_key)
    }

    pub async fn fetch_file(&self, file: &LCFile) -> Result<Vec<u8>> {
        Ok(self.http.get(&file.url).send().await?.bytes().await?.to_vec())
    }

    pub async fn query<T: LCObject>(&self) -> Result<Vec<T>> {
        parse_lc_many(self.get(format!("/classes/{}", T::CLASS_NAME))).await
    }
}
