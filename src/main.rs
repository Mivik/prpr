use anyhow::Result;
use macroquad::window::Conf;

fn build_conf() -> Conf {
    Conf {
        window_title: "prpr".to_string(),
        window_width: 1080,
        window_height: 608,
        ..Default::default()
    }
}

#[macroquad::main(build_conf)]
async fn main() -> Result<()> {
    prpr::the_main().await
}
