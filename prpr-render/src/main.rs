mod scene;

use crate::scene::MainScene;
use anyhow::{bail, Context, Result};
use kira::sound::static_sound::{StaticSoundData, StaticSoundSettings};
use macroquad::{miniquad::TextureFormat, prelude::*};
use prpr::{
    audio::AudioClip,
    build_conf,
    config::Config,
    core::{init_assets, MSRenderTarget, NoteKind},
    fs::{self, PatchedFileSystem},
    scene::{GameScene, LoadingScene, BILLBOARD},
    time::TimeManager,
    ui::{ChartInfoEdit, Ui},
    Main,
};
use std::{
    cell::RefCell,
    io::{BufWriter, Cursor, Write},
    ops::{Deref, DerefMut},
    process::{Command, Stdio},
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Instant,
};
use std::{fmt::Write as _, path::Path};

#[derive(Clone)]
struct VideoConfig {
    fps: u32,
    resolution: (u32, u32),
    hardware_accel: bool,
    ending_length: f64,
    bitrate: String,
}

impl Default for VideoConfig {
    fn default() -> Self {
        Self {
            fps: 60,
            resolution: (1920, 1080),
            hardware_accel: false,
            ending_length: 27.5,
            bitrate: "7M".to_string(),
        }
    }
}

static INFO_EDIT: Mutex<Option<ChartInfoEdit>> = Mutex::new(None);
static VIDEO_CONFIG: Mutex<Option<VideoConfig>> = Mutex::new(None);

#[cfg(target_arch = "wasm32")]
compile_error!("WASM target is not supported");

async fn the_main() -> Result<()> {
    init_assets();
    set_panic_handler(|msg, backtrace| async move {
        let _ = std::fs::write("错误信息.txt", format!("发生错误：{msg}\n\n详细堆栈：\n{backtrace}"));
    });

    let ffmpeg = if cfg!(target_os = "windows") {
        let local = Path::new("ffmpeg.exe");
        if local.exists() {
            local.display().to_string()
        } else {
            "ffmpeg".to_owned()
        }
    } else {
        "ffmpeg".to_owned()
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

    let mut fs = fs::fs_from_file(std::path::Path::new(&path))?;
    let info = fs::load_info(fs.deref_mut()).await?;

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
        if main.scenes.len() == 1 {
            gl.quad_gl.viewport(Some((0, 0, texture.width as _, texture.height as _)));
            let sw = screen_width();
            let lf = (sw - width) / 2.;
            main.update_with_mutate(|touch| {
                touch.position.x -= lf / texture.width as f32 * 2.;
            })?;
            main.show_billboard = false;
            main.render(&mut Ui::new())?;
            gl.flush();
            set_camera(&Camera2D {
                zoom: vec2(1., -screen_width() / screen_height()),
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
            main.update()?;
            gl.quad_gl.viewport(None);
            gl.quad_gl.render_pass(None);
            main.render(&mut Ui::new())?;
        }
        if main.should_exit() {
            break;
        }

        next_frame().await;
    }
    clear_background(BLACK);
    next_frame().await;

    let edit = INFO_EDIT.lock().unwrap().take().unwrap();
    let volume_music = config.volume_music;
    let volume_sfx = config.volume_sfx;
    let config = Config {
        autoplay: true,
        volume_music: 0.,
        volume_sfx: 0.,
        ..config
    };

    let v_config = VIDEO_CONFIG.lock().unwrap().take().unwrap();
    let (vw, vh) = v_config.resolution;

    let length = track_length - chart.offset.min(0.) as f64 + 1.;
    let video_length = O + length + A + v_config.ending_length;
    let offset = chart.offset.max(0.);

    let render_start_time = Instant::now();

    info!("[1] 混音中...");
    let sample_rate = 44100;
    assert_eq!(sample_rate, ending.sample_rate);
    assert_eq!(sample_rate, sfx_click.sample_rate);
    assert_eq!(sample_rate, sfx_drag.sample_rate);
    assert_eq!(sample_rate, sfx_flick.sample_rate);
    let mut output = vec![0.; (video_length * sample_rate as f64).ceil() as usize * 2];
    {
        let pos = O - chart.offset.min(0.) as f64;
        let count = (music.duration().as_secs_f64() * sample_rate as f64) as usize;
        let frames = music.frames.deref();
        let mut it = output[((pos * sample_rate as f64).round() as usize * 2)..].iter_mut();
        let ratio = music.sample_rate as f64 / sample_rate as f64;
        for frame in 0..count {
            let position = (frame as f64 * ratio).round() as usize;
            let frame = frames[position];
            *it.next().unwrap() += frame.left * volume_music;
            *it.next().unwrap() += frame.right * volume_music;
        }
    }
    let mut place = |pos: f64, clip: &AudioClip, volume: f32| {
        let position = (pos * sample_rate as f64).round() as usize * 2;
        let slice = &mut output[position..];
        let len = (slice.len() / 2).min(clip.frames.len());
        let mut it = slice.iter_mut();
        // TODO optimize?
        for frame in clip.frames[..len].iter() {
            let dst = it.next().unwrap();
            *dst += frame.left * volume;
            let dst = it.next().unwrap();
            *dst += frame.right * volume;
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
            volume_sfx,
        )
    }
    place(O + length + A, &ending, volume_music);
    let mut proc = Command::new(&ffmpeg)
        .args("-y -f f32le -ar 44100 -ac 2 -i - -c:a mp3 t_audio.mp3".split_whitespace())
        .stdin(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("无法执行 ffmpeg")?;
    let input = proc.stdin.as_mut().unwrap();
    let mut writer = BufWriter::new(input);
    for sample in output.into_iter() {
        writer.write_all(&sample.to_le_bytes())?;
    }
    drop(writer);
    proc.wait()?;

    info!("[2] 渲染视频…");
    let mst = Rc::new(MSRenderTarget::new((vw, vh), config.sample_count));
    let my_time: Rc<RefCell<f64>> = Rc::new(RefCell::new(0.));
    let tm = TimeManager::manual(Box::new({
        let my_time = Rc::clone(&my_time);
        move || *(*my_time).borrow()
    }));
    let fs = Box::new(PatchedFileSystem(fs, edit.to_patches().await?));
    static MSAA: AtomicBool = AtomicBool::new(false);
    let mut main = Main::new(Box::new(LoadingScene::new(edit.info, config, fs, None, Some(Rc::new(move || (vw, vh)))).await?), tm, {
        let mut cnt = 0;
        let mst = Rc::clone(&mst);
        move || {
            cnt += 1;
            if cnt == 1 || cnt == 3 {
                MSAA.store(true, Ordering::SeqCst);
                Some(mst.input())
            } else {
                MSAA.store(false, Ordering::SeqCst);
                Some(mst.output())
            }
        }
    })?;
    main.show_billboard = false;

    const O: f64 = LoadingScene::TOTAL_TIME as f64 + GameScene::BEFORE_TIME as f64;
    const A: f64 = 0.7 + 0.3 + 0.4;

    let fps = v_config.fps;
    let frame_delta = 1. / fps as f32;

    let codecs = String::from_utf8(Command::new(&ffmpeg).arg("-codecs").output().context("无法执行 ffmpeg")?.stdout)?;
    let use_cuda = v_config.hardware_accel && codecs.contains("h264_nvenc");
    let has_qsv = v_config.hardware_accel && codecs.contains("h264_qsv");

    let mut args = "-y -f rawvideo -c:v rawvideo".to_owned();
    if use_cuda {
        args += " -hwaccel_output_format cuda";
    }
    write!(
        &mut args,
        " -s {vw}x{vh} -r {fps} -pix_fmt rgb24 -i - -i t_audio.mp3 -c:a copy -c:v {} -map 0:v:0 -map 1:a:0 -qp 0 -vf vflip t_video.mp4",
        if use_cuda {
            "h264_nvenc"
        } else if has_qsv {
            "h264_qsv"
        } else if v_config.hardware_accel {
            bail!("不支持硬件加速！");
        } else {
            "libx264 -preset ultrafast"
        },
    )?;

    let mut proc = Command::new(&ffmpeg)
        .args(args.split_whitespace())
        .stdin(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("无法执行 ffmpeg")?;
    let mut input = proc.stdin.take().unwrap();

    let mut bytes = vec![0; vw as usize * vh as usize * 3];

    let frames = (video_length / frame_delta as f64).ceil() as u64;
    let start_time = Instant::now();

    for frame in 0..frames {
        *my_time.borrow_mut() = (frame as f32 * frame_delta).max(0.) as f64;
        gl.quad_gl.render_pass(Some(mst.output().render_pass));
        clear_background(BLACK);
        main.update()?;
        main.render(&mut Ui::new())?;
        // TODO magic. can't remove this line.
        draw_rectangle(0., 0., 0., 0., Color::default());
        gl.flush();

        if MSAA.load(Ordering::SeqCst) {
            mst.blit();
        }
        mst.output().texture.raw_miniquad_texture_handle().read_pixels(&mut bytes);
        input.write_all(&bytes)?;
        if frame % 100 == 0 {
            info!("{frame} / {frames}, {:.2}fps", frame as f64 / start_time.elapsed().as_secs_f64());
        }
    }
    drop(input);
    proc.wait()?;

    info!("[3] 合并 & 转码 & 压制");
    let _ = Command::new(&ffmpeg)
        .args("-y -i t_video.mp4 -c:a copy -pix_fmt yuv420p -b:v".split_whitespace())
        .arg(v_config.bitrate)
        .arg("out.mp4")
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .context("无法执行 ffmpeg")?;

    info!("渲染完成！耗时：{:.2}s", render_start_time.elapsed().as_secs_f64());
    Ok(())
}

#[macroquad::main(build_conf)]
async fn main() {
    if let Err(err) = the_main().await {
        let _ = std::fs::write("错误信息.txt", format!("发生错误：{err:?}"));
    }
}
