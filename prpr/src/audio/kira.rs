use super::{Audio, PlayParams};
use anyhow::Result;
use kira::{
    manager::{backend::cpal::CpalBackend, AudioManager, AudioManagerSettings, Capacities},
    sound::static_sound::{PlaybackState, StaticSoundData, StaticSoundHandle, StaticSoundSettings},
    tween::Tween,
    LoopBehavior,
};
use std::io::Cursor;

pub struct KiraAudio(pub AudioManager<CpalBackend>);

impl Audio for KiraAudio {
    type Clip = StaticSoundData;
    type Handle = StaticSoundHandle;

    fn new() -> Result<Self> {
        Ok(Self(AudioManager::new(AudioManagerSettings {
            capacities: Capacities {
                sound_capacity: 2048,
                command_capacity: 2048,
                ..Default::default()
            },
            ..Default::default()
        })?))
    }

    fn create_clip(&self, data: Vec<u8>) -> Result<(Self::Clip, f64)> {
        let data = StaticSoundData::from_cursor(Cursor::new(data), StaticSoundSettings::default())?;
        let length = data.frames.len() as f64 / data.sample_rate as f64;
        Ok((data, length))
    }

    fn play(&mut self, clip: &Self::Clip, params: PlayParams) -> Result<Self::Handle> {
        Ok(self.0.play(clip.with_modified_settings(|it| {
            it.volume(params.volume)
                .loop_behavior(if params.loop_ { Some(LoopBehavior { start_position: 0. }) } else { None })
                .playback_rate(params.playback_rate)
                .start_position(params.offset)
        }))?)
    }

    fn pause(&mut self, handle: &mut Self::Handle) -> Result<()> {
        Ok(handle.pause(Tween::default())?)
    }

    fn resume(&mut self, handle: &mut Self::Handle) -> Result<()> {
        Ok(handle.resume(Tween::default())?)
    }

    fn paused(&self, handle: &Self::Handle) -> Result<bool> {
        Ok(handle.state() == PlaybackState::Paused)
    }

    fn position(&self, handle: &Self::Handle) -> Result<f64> {
        Ok(handle.position())
    }

    fn seek_to(&mut self, handle: &mut Self::Handle, position: f64) -> Result<()> {
        Ok(handle.seek_to(position)?)
    }
}
