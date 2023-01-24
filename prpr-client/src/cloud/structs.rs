use super::LCObject;
use crate::data::BriefChartInfo;
use chrono::{Utc, DateTime};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
enum FileTypeField {
    File,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct LCFile {
    #[serde(rename = "__type")]
    type_: Option<FileTypeField>,
    #[serde(rename = "objectId")]
    pub id: String,
    pub url: String,
}

impl LCFile {
    pub fn new(id: String, url: String) -> Self {
        Self {
            type_: Some(FileTypeField::File),
            id,
            url,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum PointerTypeField {
    Pointer,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pointer {
    #[serde(rename = "__type")]
    type_: Option<PointerTypeField>,
    #[serde(rename = "objectId")]
    pub id: String,
    pub class_name: Option<String>,
}

impl From<String> for Pointer {
    fn from(id: String) -> Self {
        Self {
            type_: Some(PointerTypeField::Pointer),
            id,
            class_name: None,
        }
    }
}

impl Pointer {
    pub fn with_class<T: LCObject>(mut self) -> Self {
        self.class_name = Some(T::CLASS_NAME.to_owned());
        self
    }

    pub fn with_class_name(mut self, name: impl Into<String>) -> Self {
        self.class_name = Some(name.into());
        self
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct User {
    #[serde(rename = "objectId")]
    pub id: String,
    #[serde(rename = "username")]
    pub name: String,
    pub session_token: Option<String>,
    pub avatar: Option<LCFile>,
    pub short_id: String,
    pub email: String,
}

impl LCObject for User {
    const CLASS_NAME: &'static str = "_User";
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LCChartItem {
    #[serde(rename = "objectId")]
    pub id: Option<String>,

    #[serde(flatten)]
    pub info: BriefChartInfo,

    pub file: LCFile,
    pub illustration: LCFile,
}

impl LCObject for LCChartItem {
    const CLASS_NAME: &'static str = "Chart";
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub title: String,
    pub content: String,
    pub author: String,
    pub updated_at: DateTime<Utc>,
}

impl LCObject for Message {
    const CLASS_NAME: &'static str = "Message";
}
