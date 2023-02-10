mod objects;
pub use objects::*;

use anyhow::Result;
use reqwest::{Method, RequestBuilder};
use serde::{Deserialize, Serialize};
use serde_json::json;

pub struct Client {}

const API_URL: &str = "https://api.phi.zone";

impl Client {
    #[inline]
    pub fn get(path: impl AsRef<str>) -> RequestBuilder {
        Self::request(Method::GET, path)
    }

    #[inline]
    pub fn post<T: Serialize>(path: impl AsRef<str>, data: &T) -> RequestBuilder {
        Self::request(Method::POST, path).json(data)
    }

    #[inline]
    pub fn put<T: Serialize>(path: impl AsRef<str>, data: &T) -> RequestBuilder {
        Self::request(Method::PUT, path).json(data)
    }

    pub fn request(method: Method, path: impl AsRef<str>) -> RequestBuilder {
        reqwest::Client::new().request(method, API_URL.to_string() + path.as_ref())
    }

    pub async fn fetch<T: PZObject>(id: usize) -> Result<T> {
        Ok(Self::get(format!("/{}/{id}/", T::QUERY_PATH))
            .send()
            .await?
            .json()
            .await?)
    }

    pub async fn query<T: PZObject>(page: usize) -> Result<(Vec<T>, bool)> {
        #[derive(Deserialize)]
        struct Resp<T> {
            next: Option<String>,
            results: Vec<T>,
        }
        let resp: Resp<T> = Self::get(format!("/{}/", T::QUERY_PATH))
            .query(&json!({
                "page": page + 1,
            }))
            .send()
            .await?
            .json()
            .await?;
        Ok((resp.results, resp.next.is_some()))
    }

    pub async fn register(email: String, username: String, password: String) -> Result<()> {
        Client::post(
            "/register",
            &json!({
                "email": email,
                "username": username,
                "password": password
            }),
        )
        .send()
        .await?;
        Ok(())
    }
}
