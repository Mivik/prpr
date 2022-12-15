use anyhow::Result;

pub struct PlayParams {
    pub volume: f64,
    pub playback_rate: f64,
    pub offset: f64,
    pub loop_: bool,
}

impl Default for PlayParams {
    fn default() -> Self {
        Self {
            volume: 1.,
            playback_rate: 1.,
            offset: 0.,
            loop_: false,
        }
    }
}

pub trait Audio: Sized {
    type Clip: Clone;
    type Handle;

    fn new() -> Result<Self>;
    fn create_clip(&self, data: Vec<u8>) -> Result<(Self::Clip, f64)>;
    fn position(&self, handle: &Self::Handle) -> Result<f64>;
    fn paused(&self, handle: &Self::Handle) -> Result<bool>;
    fn play(&mut self, clip: &Self::Clip, params: PlayParams) -> Result<Self::Handle>;
    fn pause(&mut self, handle: &mut Self::Handle) -> Result<()>;
    fn resume(&mut self, handle: &mut Self::Handle) -> Result<()>;
    fn seek_to(&mut self, handle: &mut Self::Handle, position: f64) -> Result<()>;
}

pub struct DummyAudio;

impl Audio for DummyAudio {
    type Clip = ();
    type Handle = ();

    fn new() -> Result<Self> {
        Ok(Self)
    }
    fn create_clip(&self, _: Vec<u8>) -> Result<(Self::Clip, f64)> {
        Ok(((), 0.))
    }
    fn position(&self, _: &Self::Handle) -> Result<f64> {
        Ok(0.0)
    }
    fn paused(&self, _: &Self::Handle) -> Result<bool> {
        Ok(false)
    }
    fn play(&mut self, _: &Self::Clip, _: PlayParams) -> Result<Self::Handle> {
        Ok(())
    }
    fn pause(&mut self, _: &mut Self::Handle) -> Result<()> {
        Ok(())
    }
    fn resume(&mut self, _: &mut Self::Handle) -> Result<()> {
        Ok(())
    }
    fn seek_to(&mut self, _: &mut Self::Handle, _: f64) -> Result<()> {
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod kira;
#[cfg(not(target_arch = "wasm32"))]
pub type DefaultAudio = kira::KiraAudio;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub type DefaultAudio = web::WebAudio;

// pub type DefaultAudio = DummyAudio;

pub type AudioClip = <DefaultAudio as Audio>::Clip;
pub type AudioHandle = <DefaultAudio as Audio>::Handle;
