use anyhow::Result;
use prpr::build_conf;

#[macroquad::main(build_conf)]
async fn main() -> Result<()> {
    prpr::the_main().await
}
