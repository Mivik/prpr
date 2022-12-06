use super::Audio;
use anyhow::Result;
use kira::{
    manager::{backend::cpal::CpalBackend, AudioManager, AudioManagerSettings},
    sound::static_sound::{PlaybackState, StaticSoundData, StaticSoundHandle, StaticSoundSettings},
    tween::Tween,
};
use std::io::Cursor;

pub struct KiraAudio(AudioManager<CpalBackend>);

impl Audio for KiraAudio {
    type Clip = StaticSoundData;
    type Handle = StaticSoundHandle;

    fn new() -> Result<Self> {
        Ok(Self(AudioManager::new(AudioManagerSettings::default())?))
    }

    fn create_clip(&self, data: Vec<u8>) -> Result<Self::Clip> {
        Ok(StaticSoundData::from_cursor(
            Cursor::new(data),
            StaticSoundSettings::default(),
        )?)
    }

    fn play(&mut self, clip: &Self::Clip, volume: f64, offset: f64) -> Result<Self::Handle> {
        Ok(self
            .0
            .play(clip.with_modified_settings(|it| it.start_position(offset).volume(volume)))?)
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
