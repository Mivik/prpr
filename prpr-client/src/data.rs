use crate::{
    dir,
    page::ChartItem, phizone::{Ptr, PZUser},
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use prpr::{config::Config, info::ChartInfo};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, ops::DerefMut, path::Path};

pub const THEMES: [(&str, u32, u32); 8] = [
    ("Aurora", 0xff303f9f, 0xff1976d2),
    ("Flamingo", 0xffd500f9, 0xffffa000),
    ("Forest", 0xff2e7d32, 0xff4db6ac),
    ("Lively", 0xffbf360c, 0xfff57c00),
    ("Mythical", 0xff7b1fa2, 0xfffc3dff),
    ("Ocean", 0xff303f9f, 0xff1976d2),
    ("Spring", 0xfffff9c4, 0xff64ffda),
    ("Classic", 0xffa2a2a2, 0xffa2a2a2),
];

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BriefChartInfo {
    pub id: Option<u64>,
    pub uploader: Option<Ptr<PZUser>>,
    pub name: String,
    pub level: String,
    pub difficulty: f32,
    #[serde(alias = "preview_time")]
    pub preview_start: f32,
    pub preview_end: Option<f32>,
    pub intro: String,
    pub tags: Vec<String>,
    pub composer: String,
    pub illustrator: String,
}

impl From<ChartInfo> for BriefChartInfo {
    fn from(info: ChartInfo) -> Self {
        Self {
            id: info.id,
            uploader: None,
            name: info.name,
            level: info.level,
            difficulty: info.difficulty,
            preview_start: info.preview_start,
            preview_end: info.preview_end,
            intro: info.intro,
            tags: info.tags,
            composer: info.composer,
            illustrator: info.illustrator,
        }
    }
}

impl BriefChartInfo {
    pub fn into_full(self) -> ChartInfo {
        ChartInfo {
            id: self.id,
            name: self.name,
            level: self.level,
            difficulty: self.difficulty,
            preview_start: self.preview_start,
            preview_end: self.preview_end,
            intro: self.intro,
            tags: self.tags,
            composer: self.composer,
            illustrator: self.illustrator,
            ..Default::default()
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct LocalChart {
    #[serde(flatten)]
    pub info: BriefChartInfo,
    pub path: String,
}

#[derive(Default, Serialize, Deserialize)]
pub struct Data {
    pub me: Option<PZUser>,
    pub charts: Vec<LocalChart>,
    pub config: Config,
    pub message_check_time: Option<DateTime<Utc>>,
    pub language: Option<String>,
    pub theme: usize,
    pub tokens: Option<(String, String)>,
}

impl Data {
    pub async fn init(&mut self) -> Result<()> {
        let charts = dir::charts()?;
        self.charts.retain(|it| Path::new(&format!("{}/{}", charts, it.path)).exists());
        let occurred: HashSet<_> = self.charts.iter().map(|it| it.path.clone()).collect();
        for entry in std::fs::read_dir(dir::custom_charts()?)? {
            let entry = entry?;
            let filename = entry.file_name();
            let filename = filename.to_str().unwrap();
            let filename = format!("custom/{filename}");
            if occurred.contains(&filename) {
                continue;
            }
            let path = entry.path();
            let Ok(mut fs) = prpr::fs::fs_from_file(&path) else {
                continue;
            };
            let result = prpr::fs::load_info(fs.deref_mut()).await;
            if let Ok(info) = result {
                self.charts.push(LocalChart {
                    info: BriefChartInfo { id: None, ..info.into() },
                    path: filename,
                });
            }
        }
        if let Some(res_pack_path) = &mut self.config.res_pack_path {
            if res_pack_path.starts_with('/') {
                // for compatibility
                *res_pack_path = "chart.zip".to_owned();
            }
        }
        Ok(())
    }

    pub fn find_chart(&self, chart: &ChartItem) -> Option<usize> {
        self.charts.iter().position(|it| it.path == chart.path)
    }
}
