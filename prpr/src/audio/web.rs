use super::{Audio, PlayParams};
use anyhow::{anyhow, bail, Error, Result};
use std::io::Cursor;
use symphonia::core::{
    audio::{AudioBufferRef, Signal},
    io::MediaSourceStream,
};
use wasm_bindgen::JsValue;
use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext};

pub struct WebAudio(AudioContext);

fn js_err(err: JsValue) -> Error {
    Error::msg(format!("{err:?}"))
}

enum AudioState {
    Playing(f64), // playing from
    Paused(f64),  // paused at
}

pub struct AudioHandle(AudioBufferSourceNode, AudioState, AudioBuffer, PlayParams);

fn load_frames_from_buffer(channels: &mut [Vec<f32>; 2], buffer: &symphonia::core::audio::AudioBuffer<f32>) {
    for i in 0..buffer.spec().channels.count().min(2) {
        channels[i].extend_from_slice(buffer.chan(i));
    }
}

fn load_frames_from_buffer_ref(channels: &mut [Vec<f32>; 2], buffer: &AudioBufferRef) -> Result<()> {
    macro_rules! conv {
        ($buffer:ident) => {{
            let mut dest = symphonia::core::audio::AudioBuffer::new(buffer.capacity() as u64, buffer.spec().clone());
            $buffer.convert(&mut dest);
            load_frames_from_buffer(channels, &dest);
        }};
    }
    use AudioBufferRef::*;
    match buffer {
        F32(buffer) => load_frames_from_buffer(channels, buffer),
        U8(buffer) => conv!(buffer),
        U16(buffer) => conv!(buffer),
        U24(buffer) => conv!(buffer),
        U32(buffer) => conv!(buffer),
        S8(buffer) => conv!(buffer),
        S16(buffer) => conv!(buffer),
        S24(buffer) => conv!(buffer),
        S32(buffer) => conv!(buffer),
        F64(buffer) => conv!(buffer),
    }
    Ok(())
}

impl Audio for WebAudio {
    type Clip = AudioBuffer;
    type Handle = AudioHandle;

    fn new() -> Result<Self> {
        Ok(Self(AudioContext::new().map_err(js_err)?))
    }

    fn create_clip(&self, data: Vec<u8>) -> Result<(Self::Clip, f64)> {
        let codecs = symphonia::default::get_codecs();
        let probe = symphonia::default::get_probe();
        let mss = MediaSourceStream::new(Box::new(Cursor::new(data)), Default::default());
        let mut format_reader = probe.format(&Default::default(), mss, &Default::default(), &Default::default())?.format;
        let codec_params = &format_reader
            .default_track()
            .ok_or_else(|| anyhow!("Default track not found"))?
            .codec_params;
        let sample_rate = codec_params.sample_rate.ok_or_else(|| anyhow!("Unknown sample rate"))?;
        let mut decoder = codecs.make(codec_params, &Default::default())?;
        let mut channels = [vec![], vec![]];
        loop {
            match format_reader.next_packet() {
                Ok(packet) => {
                    let buffer = decoder.decode(&packet)?;
                    load_frames_from_buffer_ref(&mut channels, &buffer)?;
                }
                Err(error) => match error {
                    symphonia::core::errors::Error::IoError(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => {
                        break;
                    }
                    _ => bail!(error),
                },
            }
        }

        if !channels[1].is_empty() && channels[0].len() != channels[1].len() {
            bail!("Mixed mono and stereo output");
        }
        let stereo = !channels[1].is_empty();
        let clip = self
            .0
            .create_buffer(if stereo { 2 } else { 1 }, channels[0].len() as u32, sample_rate as f32)
            .map_err(js_err)?;
        clip.copy_to_channel(&channels[0], 0).map_err(js_err)?;
        if stereo {
            clip.copy_to_channel(&channels[1], 1).map_err(js_err)?;
        }
        Ok((clip, channels[0].len() as f64 / sample_rate as f64))
    }

    fn position(&self, handle: &Self::Handle) -> Result<f64> {
        Ok(match handle.1 {
            AudioState::Playing(start) => self.0.current_time() - start,
            AudioState::Paused(time) => time,
        })
    }

    fn paused(&self, handle: &Self::Handle) -> Result<bool> {
        Ok(matches!(handle.1, AudioState::Paused(_)))
    }

    fn play(&mut self, clip: &Self::Clip, params: PlayParams) -> Result<Self::Handle> {
        let gain = self.0.create_gain().map_err(js_err)?;
        gain.gain().set_value(params.volume as _);
        let node = self.0.create_buffer_source().map_err(js_err)?;
        node.set_buffer(Some(clip));
        node.playback_rate().set_value(params.playback_rate as f32);
        node.connect_with_audio_node(&gain).map_err(js_err)?;
        gain.connect_with_audio_node(&self.0.destination()).map_err(js_err)?;
        node.start_with_when_and_grain_offset(0., params.offset).map_err(js_err)?;
        if params.loop_ {
            node.set_loop(true);
        }
        Ok(AudioHandle(node, AudioState::Playing(self.0.current_time() - params.offset), clip.clone(), params))
    }

    fn pause(&mut self, handle: &mut Self::Handle) -> Result<()> {
        let AudioState::Playing(time) = handle.1 else {
            bail!("Pausing an already paused clip");
        };
        handle.1 = AudioState::Paused(self.0.current_time() - time);
        handle.0.stop().map_err(js_err)?;
        Ok(())
    }

    fn resume(&mut self, handle: &mut Self::Handle) -> Result<()> {
        let AudioState::Paused(time) = handle.1 else {
            bail!("Resuming an playing clip");
        };
        *handle = self.play(&handle.2, PlayParams { offset: time, ..handle.3 })?;
        Ok(())
    }

    fn seek_to(&mut self, handle: &mut Self::Handle, position: f64) -> Result<()> {
        handle.0.stop().map_err(js_err)?;
        *handle = self.play(
            &handle.2,
            PlayParams {
                offset: position,
                ..handle.3
            },
        )?;
        Ok(())
    }
}
