use anyhow::{bail, Result};
use macroquad::{miniquad::TextureFormat, prelude::*};
use prpr::{audio::AudioClip, build_conf, core::NoteKind, fs, Prpr};
use std::{
    io::{BufWriter, Write},
    process::{Command, Stdio},
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

    let path = {
        let mut args = std::env::args();
        let program = args.next().unwrap();
        let Some(path) = args.next() else {
            bail!("Usage: {program} <chart>");
        };
        path
    };

    let (mut config, fs) = fs::load_config(fs::fs_from_file(&path)?).await?;
    config.adjust_time = false;
    config.autoplay = true;
    config.volume_music = 0.;
    config.volume_sfx = 0.;

    let mut proc = Command::new("ffmpeg")
        .args(format!("-y -f rawvideo -vcodec rawvideo -s {VIDEO_WIDTH}x{VIDEO_HEIGHT} -r {FPS} -pix_fmt rgb24 -i - -threads 8 -c:v libx264 -preset ultrafast -qp 0 -vf vflip t_video.mp4").split_whitespace())
        .stdin(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let input = proc.stdin.as_mut().unwrap();

    let mut prpr = Prpr::new(config, fs, Some(Box::new(|| (VIDEO_WIDTH, VIDEO_HEIGHT)))).await?;

    let texture = miniquad::Texture::new_render_texture(
        prpr.gl.quad_context,
        miniquad::TextureParams {
            width: VIDEO_WIDTH,
            height: VIDEO_HEIGHT,
            format: TextureFormat::RGB8,
            ..Default::default()
        },
    );
    prpr.res.camera.render_target = Some({
        let render_pass = miniquad::RenderPass::new(prpr.gl.quad_context, texture, None);
        RenderTarget {
            texture: Texture2D::from_miniquad_texture(texture),
            render_pass,
        }
    });

    info!("[1] Rendering video...");
    set_camera(&prpr.res.camera);

    let mut bytes = vec![0; VIDEO_WIDTH as usize * VIDEO_HEIGHT as usize * 3];

    let length = prpr.res.track_length - prpr.chart.offset.min(0.) + 1.;
    let offset = prpr.chart.offset.max(0.);
    let frames = (length as f64 / FRAME_DELTA as f64).ceil() as u64;
    let start_time = Instant::now();
    for frame in 0..frames {
        prpr.update(Some((frame as f32 * FRAME_DELTA - offset).max(0.) as f64))?;
        prpr.render(Some(1. / 60.))?;
        prpr.ui(false)?;
        prpr.gl.flush();

        texture.read_pixels(&mut bytes);
        input.write_all(&bytes)?;
        if frame % 100 == 0 {
            info!(
                "{frame} / {frames}, {:.2}fps",
                frame as f64 / start_time.elapsed().as_secs_f64()
            );
        }
    }
    proc.wait()?;

    info!("[2] Mixing audio...");
    let sample_rate = 44100;
    assert_eq!(sample_rate, prpr.res.music.sample_rate);
    assert_eq!(sample_rate, prpr.res.sfx_click.sample_rate);
    assert_eq!(sample_rate, prpr.res.sfx_drag.sample_rate);
    assert_eq!(sample_rate, prpr.res.sfx_flick.sample_rate);
    let mut output = vec![0.; (length as f64 * sample_rate as f64).ceil() as usize * 2];
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
    place(-prpr.chart.offset.min(0.) as f64, &prpr.res.music);
    for note in prpr
        .chart
        .lines
        .iter()
        .flat_map(|it| it.notes.iter())
        .filter(|it| !it.fake)
    {
        place(
            note.time as f64 + offset as f64,
            match note.kind {
                NoteKind::Click | NoteKind::Hold { .. } => &prpr.res.sfx_click,
                NoteKind::Drag => &prpr.res.sfx_drag,
                NoteKind::Flick => &prpr.res.sfx_flick,
            },
        )
    }

    info!("[3] Merging...");
    let mut proc = Command::new("ffmpeg")
        .args("-y -i t_video.mp4 -f f32le -ar 44100 -ac 2 -i - -af loudnorm -c:v copy -c:a mp3 -map 0:v:0 -map 1:a:0 out.mp4".to_string().split_whitespace())
        .stdin(Stdio::piped())
        .stderr(Stdio::null())
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
