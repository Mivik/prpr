use super::UploadToken;
use anyhow::{bail, Context, Result};
use reqwest::header;
use serde::Serialize;
use serde_json::{json, Value};

const SIZE: usize = 4 * 1024 * 1024;

pub(super) async fn upload_qiniu(token: UploadToken, data: &[u8]) -> Result<()> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct QiniuPart {
        part_number: usize,
        etag: String,
    }
    let encoded_name = base64::encode(token.key.as_bytes());
    let auth = format!("UpToken {}", token.token);
    let client = reqwest::Client::new();
    let prefix = format!("{}/buckets/{}/objects/{encoded_name}/uploads", token.upload_url, token.bucket);
    let upload_id = {
        let resp = client
            .post(&prefix)
            .header(header::AUTHORIZATION, &auth)
            .send()
            .await
            .context("Failed to request upload id")?;
        let status = resp.status();
        let text = resp.text().await.context("Failed to receive text")?;
        if !status.is_success() {
            bail!("Failed to request upload id: {text}");
        }
        let value: Value = serde_json::from_str(&text)?;
        value["uploadId"].as_str().unwrap().to_owned()
    };
    let prefix = format!("{prefix}/{upload_id}");
    let mut parts = Vec::new();
    for (id, chunk) in data.chunks(SIZE).enumerate() {
        let id = id + 1;
        let resp = client
            .put(format!("{prefix}/{}", id))
            .header(header::AUTHORIZATION, &auth)
            .header(header::CONTENT_TYPE, "application/octet-stream")
            .header("Content-MD5", base64::encode(md5::compute(chunk).0))
            .body(chunk.to_owned())
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await.context("Failed to receive text")?;
        if !status.is_success() {
            bail!("Failed to upload file: {text}");
        }
        let value: Value = serde_json::from_str(&text)?;
        parts.push(QiniuPart {
            part_number: id,
            etag: value["etag"].as_str().unwrap().to_owned(),
        });
    }
    let resp = client
        .post(prefix)
        .header(header::AUTHORIZATION, &auth)
        .header(header::CONTENT_TYPE, "application/json")
        .body(serde_json::to_string(&json!({ "parts": parts }))?)
        .send()
        .await?;
    if !resp.status().is_success() {
        bail!("Failed to upload file: {}", resp.text().await?);
    }
    Ok(())
}
