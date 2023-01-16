use crate::{
    cloud::{Pointer, User},
    dir,
};
use anyhow::Result;
use prpr::{config::Config, info::ChartInfo};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, ops::DerefMut, path::Path};

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

impl BriefChartInfo {
    pub fn into_full(self) -> ChartInfo {
        ChartInfo {
            id: self.id,
            name: self.name,
            level: self.level,
            difficulty: self.difficulty,
            preview_time: self.preview_time,
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
    pub me: Option<User>,
    charts: Vec<LocalChart>,
    pub config: Config,
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
        Ok(())
    }

    pub fn add_chart(&mut self, chart: LocalChart) {
        self.charts.push(chart);
    }

    pub fn remove_chart(&mut self, id: usize) {
        self.charts.remove(self.charts.len() - id - 1);
    }

    pub fn charts(&self) -> impl Iterator<Item = &LocalChart> {
        self.charts.iter().rev()
    }

    pub fn chart(&self, id: usize) -> &LocalChart {
        let id = self.charts.len() - id - 1;
        &self.charts[id]
    }

    pub fn chart_mut(&mut self, id: usize) -> &mut LocalChart {
        let id = self.charts.len() - id - 1;
        &mut self.charts[id]
    }
}
