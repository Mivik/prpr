pub enum ChartOrder {
    Default,
    Name,
}

impl ChartOrder {
    pub fn apply(&self, charts: &mut Vec<ChartItem>) {
        match self {
            Self::Default => {
                charts.reverse();
            }
            Self::Name => {
                charts.sort_by(|x, y| x.info.name.cmp(&y.info.name));
            }
        }
    }
}