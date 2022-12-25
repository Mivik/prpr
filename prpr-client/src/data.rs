use crate::{
    cloud::{Pointer, User},
    dir,
};
use anyhow::Result;
use prpr::{config::Config, info::ChartInfo};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BriefChartInfo {
    pub id: Option<String>,
    pub uploader: Option<Pointer>,
    pub name: String,
    pub level: String,
    pub difficulty: f32,
    pub preview_time: f32,
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
            preview_time: info.preview_time,
            intro: info.intro,
            tags: info.tags,
            composer: info.composer,
            illustrator: info.illustrator,
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
    pub me: Option<User>,
    pub charts: Vec<LocalChart>,
    pub config: Config,
}

impl Data {
    pub async fn init(&mut self) -> Result<()> {
        let charts = dir::charts()?;
        self.charts.retain(|it| std::fs::metadata(format!("{}/{}", charts, it.path)).is_ok());
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
            let fs = prpr::fs::fs_from_file(&path)?;
            let result = prpr::fs::load_info(fs).await;
            if let Ok((info, _)) = result {
                self.charts.push(LocalChart {
                    info: BriefChartInfo { id: None, ..info.into() },
                    path: filename,
                })
            }
        }
        Ok(())
    }
}