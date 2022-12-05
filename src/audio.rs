use anyhow::Result;
use std::{future::Future, pin::Pin};

pub trait Audio: Sized {
    type Clip;
    type Handle;

    fn new() -> Result<Self>;
    fn create_clip(
        &self,
        data: Vec<u8>,
    ) -> Result<Pin<Box<dyn Future<Output = Result<Self::Clip>>>>>;
    fn position(&self, handle: &Self::Handle) -> Result<f64>;
    fn paused(&self, handle: &Self::Handle) -> Result<bool>;
    fn play(&mut self, clip: &Self::Clip, volume: f64, offset: f64) -> Result<Self::Handle>;
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
    fn create_clip(&self, _: Vec<u8>) -> Result<Pin<Box<dyn Future<Output = Result<Self::Clip>>>>> {
        Ok(Box::pin(std::future::ready(Ok(()))))
    }
    fn position(&self, _: &Self::Handle) -> Result<f64> {
        Ok(0.0)
    }
    fn paused(&self, _: &Self::Handle) -> Result<bool> {
        Ok(false)
    }
    fn play(&mut self, _: &Self::Clip, _: f64, _: f64) -> Result<Self::Handle> {
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

pub type AudioClip = <DefaultAudio as Audio>::Clip;
pub type AudioHandle = <DefaultAudio as Audio>::Handle;
