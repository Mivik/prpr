use anyhow::{bail, Result, Context};
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use macroquad::{miniquad::TextureFormat, prelude::*};
use prpr::{
    audio::AudioClip,
    build_conf,
    config::Config,
    core::NoteKind,
    fs,
    scene::{GameScene, LoadingScene},
    time::TimeManager,
    Main,
};
use std::{
    cell::RefCell,
    io::{BufWriter, Cursor, Write},
    process::{Command, Stdio},
    rc::Rc,
    time::Instant,
};

const FPS: u32 = 60;
const FRAME_DELTA: f32 = 1. / FPS as f32;
const VIDEO_WIDTH: u32 = 1920;
const VIDEO_HEIGHT: u32 = 1080;

#[cfg(target_arch = "wasm32")]
compile_error!("WASM target is not supported");

#[macroquad::main(build_conf)]
async fn main() -> Result<()> {
    set_pc_assets_folder("assets");

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let _guard = rt.enter();

    let _ = prpr::ui::FONT.set(load_ttf_font("font.ttf").await?);

    let (path, config) = {
        let mut args = std::env::args();
        let program = args.next().unwrap();
        let Some(path) = args.next() else {
            bail!("Usage: {program} <chart>");
        };
        let mut config = Config::default();
        if let Some(config_path) = args.next() {
            config = serde_yaml::from_str(&std::fs::read_to_string(config_path).context("Cannot read from config file")?)?;
        }
        (path, config)
    };

    let (info, mut fs) = fs::load_info(fs::fs_from_file(std::path::Path::new(&path))?).await?;

    let chart = GameScene::load_chart(&mut fs, &info).await?;
    macro_rules! ld {
        ($path:literal) => {
            StaticSoundData::from_cursor(Cursor::new(load_file($path).await?), StaticSoundSettings::default())?
        };
    }
    let music = StaticSoundData::from_cursor(Cursor::new(fs.load_file(&info.music).await?), StaticSoundSettings::default())?;
    let ending = StaticSoundData::from_cursor(Cursor::new(load_file("ending.mp3").await?), StaticSoundSettings::default())?;
    let track_length = music.frames.len() as f64 / music.sample_rate as f64;
    let sfx_click = ld!("click.ogg");
    let sfx_drag = ld!("drag.ogg");
    let sfx_flick = ld!("flick.ogg");

    let config = Config {
        autoplay: true,
        volume_music: 0.,
        volume_sfx: 0.,
        ..config
    };

    let mut proc = Command::new("ffmpeg")
        .args(format!("-y -f rawvideo -vcodec rawvideo -s {VIDEO_WIDTH}x{VIDEO_HEIGHT} -r {FPS} -pix_fmt rgb24 -i - -threads 8 -c:v libx264 -preset ultrafast -qp 0 -vf vflip t_video.mp4").split_whitespace())
        .stdin(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let input = proc.stdin.as_mut().unwrap();

    let mut gl = unsafe { get_internal_gl() };
    let texture = miniquad::Texture::new_render_texture(
        gl.quad_context,
        miniquad::TextureParams {
            width: VIDEO_WIDTH,
            height: VIDEO_HEIGHT,
            format: TextureFormat::RGB8,
            ..Default::default()
        },
    );
    let target = Some({
        let render_pass = miniquad::RenderPass::new(gl.quad_context, texture, None);
        RenderTarget {
            texture: Texture2D::from_miniquad_texture(texture),
            render_pass,
        }
    });

    info!("[1] Rendering video...");

    let my_time: Rc<RefCell<f64>> = Rc::new(RefCell::new(0.));
    let tm = TimeManager::manual(Box::new({
        let my_time = Rc::clone(&my_time);
        move || *(*my_time).borrow()
    }));
    let mut main = Main::new(Box::new(LoadingScene::new(info, config, fs, Some(Rc::new(|| (VIDEO_WIDTH, VIDEO_HEIGHT)))).await?), tm, target)?;

    let mut bytes = vec![0; VIDEO_WIDTH as usize * VIDEO_HEIGHT as usize * 3];

    const O: f64 = LoadingScene::TOTAL_TIME as f64 + GameScene::BEFORE_TIME as f64;
    const A: f64 = 0.7 + 0.3 + 0.4;

    let length = track_length - chart.offset.min(0.) as f64 + 1.;
    let offset = chart.offset.max(0.);
    let frames = ((O + length + A + ending.frames.len() as f64 / ending.sample_rate as f64) / FRAME_DELTA as f64).ceil() as u64;
    let start_time = Instant::now();
    for frame in 0..frames {
        *my_time.borrow_mut() = (frame as f32 * FRAME_DELTA).max(0.) as f64;
        main.update()?;
        main.render()?;
        gl.flush();

        texture.read_pixels(&mut bytes);
        input.write_all(&bytes)?;
        if frame % 100 == 0 {
            info!("{frame} / {frames}, {:.2}fps", frame as f64 / start_time.elapsed().as_secs_f64());
        }
    }
    proc.wait()?;

    info!("[2] Mixing audio...");
    let sample_rate = 44100;
    assert_eq!(sample_rate, music.sample_rate);
    assert_eq!(sample_rate, ending.sample_rate);
    assert_eq!(sample_rate, sfx_click.sample_rate);
    assert_eq!(sample_rate, sfx_drag.sample_rate);
    assert_eq!(sample_rate, sfx_flick.sample_rate);
    let mut output = vec![0.; ((O + length + A + ending.frames.len() as f64 / sample_rate as f64) * sample_rate as f64).ceil() as usize * 2];
    let mut place = |pos: f64, clip: &AudioClip| {
        let position = (pos * sample_rate as f64).round() as usize * 2;
        let mut it = output[position..].iter_mut();
        // TODO optimize?
        for frame in clip.frames.iter() {
            let dst = it.next().unwrap();
            *dst += frame.left;
            let dst = it.next().unwrap();
            *dst += frame.right;
        }
    };
    place(O - chart.offset.min(0.) as f64, &music);
    for note in chart.lines.iter().flat_map(|it| it.notes.iter()).filter(|it| !it.fake) {
        place(
            O + note.time as f64 + offset as f64,
            match note.kind {
                NoteKind::Click | NoteKind::Hold { .. } => &sfx_click,
                NoteKind::Drag => &sfx_drag,
                NoteKind::Flick => &sfx_flick,
            },
        )
    }
    place(O + length + A, &ending);

    info!("[3] Merging...");
    let mut proc = Command::new("ffmpeg")
        .args(
            "-y -i t_video.mp4 -f f32le -ar 44100 -ac 2 -i - -af loudnorm -vf format=yuv420p -c:a mp3 -map 0:v:0 -map 1:a:0 out.mp4"
                .to_string()
                .split_whitespace(),
        )
        .stdin(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()?;
    let input = proc.stdin.as_mut().unwrap();
    let mut writer = BufWriter::new(input);
    for sample in output.into_iter() {
        writer.write_all(&sample.to_le_bytes())?;
    }
    std::fs::remove_file("t_video.mp4")?;

    info!("[4] Done!");

    Ok(())
}
