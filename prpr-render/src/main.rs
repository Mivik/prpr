mod scene;

use crate::scene::MainScene;
use anyhow::{bail, Context, Result};
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use macroquad::{miniquad::TextureFormat, prelude::*};
use prpr::{
    audio::AudioClip,
    build_conf,
    config::Config,
    core::NoteKind,
    fs,
    info::ChartInfo,
    scene::{GameScene, LoadingScene},
    time::TimeManager,
    ui::Ui,
    Main,
};
use prpr::{ext::screen_aspect, scene::BILLBOARD};
use std::{
    cell::RefCell,
    io::{BufWriter, Cursor, Write},
    ops::Deref,
    process::{Command, Stdio},
    rc::Rc,
    sync::Mutex,
    time::Instant,
};

const FPS: u32 = 60;
const FRAME_DELTA: f32 = 1. / FPS as f32;

static EDITED_INFO: Mutex<Option<ChartInfo>> = Mutex::new(None);
static VIDEO_RESOLUTION: Mutex<(u32, u32)> = Mutex::new((1920, 1080));

#[cfg(target_arch = "wasm32")]
compile_error!("WASM target is not supported");

#[macroquad::main(build_conf)]
async fn main() -> Result<()> {
    set_pc_assets_folder("assets");

    let ffmpeg = if cfg!(target_os = "windows") {
        "./ffmpeg"
    } else {
        "ffmpeg"
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(4)
        .enable_all()
        .build()
        .unwrap();
    let _guard = rt.enter();

    let _ = prpr::ui::FONT.set(load_ttf_font("font.ttf").await?);

    let (path, config) = {
        let mut args = std::env::args().skip(1);
        let Some(path) = args.next() else {
            bail!("请将谱面文件或文件夹拖动到该软件上！");
        };
        let config =
            match (|| -> Result<Config> { Ok(serde_yaml::from_str(&std::fs::read_to_string("conf.yml").context("无法加载配置文件")?)?) })() {
                Err(err) => {
                    warn!("无法加载配置文件：{:?}", err);
                    Config::default()
                }
                Ok(config) => config,
            };
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

    let mut gl = unsafe { get_internal_gl() };

    let texture = miniquad::Texture::new_render_texture(
        gl.quad_context,
        miniquad::TextureParams {
            width: 1080,
            height: 608,
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
    let tex = Texture2D::from_miniquad_texture(texture);
    let mut main = Main::new(Box::new(MainScene::new(target, info, config.clone(), fs.clone_box())), TimeManager::default(), None)?;
    let width = texture.width as f32 / 2.;
    loop {
        main.update()?;
        if main.scenes.len() == 1 {
            gl.quad_gl.viewport(Some((0, 0, texture.width as _, texture.height as _)));
            let mut ui = Ui::new();
            let sw = screen_width();
            let lf = (sw - width) / 2.;
            ui.mutate_touches(|touch| {
                touch.position.x -= lf / texture.width as f32 * 2.;
            });
            main.show_billboard = false;
            main.render(&mut ui)?;
            gl.flush();
            set_camera(&Camera2D {
                zoom: vec2(1., -screen_aspect()),
                ..Default::default()
            });
            let mut ui = Ui::new();
            clear_background(GRAY);
            draw_texture_ex(
                tex,
                -1. + lf / sw * 2.,
                -ui.top,
                WHITE,
                DrawTextureParams {
                    flip_y: true,
                    dest_size: Some(vec2(texture.width as f32, texture.height as f32) * (2. / sw)),
                    ..Default::default()
                },
            );
            BILLBOARD.with(|it| {
                let mut guard = it.borrow_mut();
                let t = guard.1.now() as f32;
                guard.0.render(&mut ui, t);
            });
        } else {
            gl.quad_gl.viewport(None);
            gl.quad_gl.render_pass(None);
            set_default_camera();
            main.render(&mut Ui::new())?;
        }
        if main.should_exit() {
            break;
        }

        next_frame().await;
    }
    clear_background(BLACK);
    next_frame().await;

    let info = EDITED_INFO.lock().unwrap().take().unwrap();
    let config = Config {
        autoplay: true,
        volume_music: 0.,
        volume_sfx: 0.,
        ..config
    };

    let (vw, vh) = VIDEO_RESOLUTION.lock().unwrap().clone();

    let texture = miniquad::Texture::new_render_texture(
        gl.quad_context,
        miniquad::TextureParams {
            width: vw,
            height: vh,
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

    info!("[1] 渲染视频…");

    let my_time: Rc<RefCell<f64>> = Rc::new(RefCell::new(0.));
    let tm = TimeManager::manual(Box::new({
        let my_time = Rc::clone(&my_time);
        move || *(*my_time).borrow()
    }));
    let mut main = Main::new(Box::new(LoadingScene::new(info, config, fs, None, Some(Rc::new(move || (vw, vh)))).await?), tm, target)?;
    main.show_billboard = false;

    let mut bytes = vec![0; vw as usize * vh as usize * 3];

    const O: f64 = LoadingScene::TOTAL_TIME as f64 + GameScene::BEFORE_TIME as f64;
    const A: f64 = 0.7 + 0.3 + 0.4;

    let mut proc = Command::new(ffmpeg)
        .args(format!("-y -f rawvideo -vcodec rawvideo -s {vw}x{vh} -r {FPS} -pix_fmt rgb24 -i - -threads 8 -c:v libx264 -preset ultrafast -qp 0 -vf vflip t_video.mp4").split_whitespace())
        .stdin(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let input = proc.stdin.as_mut().unwrap();

    let length = track_length - chart.offset.min(0.) as f64 + 1.;
    let offset = chart.offset.max(0.);
    let frames = ((O + length + A + ending.frames.len() as f64 / ending.sample_rate as f64) / FRAME_DELTA as f64).ceil() as u64;
    let start_time = Instant::now();
    for frame in 0..frames {
        *my_time.borrow_mut() = (frame as f32 * FRAME_DELTA).max(0.) as f64;
        main.update()?;
        main.render(&mut Ui::new())?;
        gl.flush();

        texture.read_pixels(&mut bytes);
        input.write_all(&bytes)?;
        if frame % 100 == 0 {
            info!("{frame} / {frames}, {:.2}fps", frame as f64 / start_time.elapsed().as_secs_f64());
        }
    }
    proc.wait()?;

    info!("[2] 混音中...");
    let sample_rate = 44100;
    assert_eq!(sample_rate, ending.sample_rate);
    assert_eq!(sample_rate, sfx_click.sample_rate);
    assert_eq!(sample_rate, sfx_drag.sample_rate);
    assert_eq!(sample_rate, sfx_flick.sample_rate);
    let mut output = vec![0.; ((O + length + A + ending.frames.len() as f64 / sample_rate as f64) * sample_rate as f64).ceil() as usize * 2];
    {
        let pos = O - chart.offset.min(0.) as f64;
        let count = (music.duration().as_secs_f64() * sample_rate as f64) as usize;
        let frames = music.frames.deref();
        let mut it = output[((pos * sample_rate as f64).round() as usize * 2)..].iter_mut();
        let ratio = music.sample_rate as f64 / sample_rate as f64;
        for frame in 0..count {
            let position = (frame as f64 * ratio).round() as usize;
            let frame = frames[position];
            *it.next().unwrap() += frame.left;
            *it.next().unwrap() += frame.right;
        }
    }
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

    info!("[3] 合并 & 压缩…");
    let mut proc = Command::new(ffmpeg)
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
    drop(writer);
    proc.wait()?;
    std::fs::remove_file("t_video.mp4")?;

    info!("[4] 完成！");

    Ok(())
}
