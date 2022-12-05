use super::Audio;
use anyhow::{bail, Error, Result};
use js_sys::Uint8Array;
use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::Poll,
};
use wasm_bindgen::{prelude::Closure, JsCast, JsValue};
use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext};

pub struct WebAudio(AudioContext);

fn js_err(err: JsValue) -> Error {
    Error::msg(format!("{err:?}"))
}

enum AudioState {
    Playing(f64), // playing from
    Paused(f64),  // paused at
}

pub struct AudioHandle(AudioBufferSourceNode, AudioState, AudioBuffer, f64);

use wasm_bindgen::prelude::*;
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &JsValue);
}

impl Audio for WebAudio {
    type Clip = AudioBuffer;
    type Handle = AudioHandle;

    fn new() -> Result<Self> {
        let nb = AudioContext::new().map_err(js_err)?;
        log(nb.as_ref());
        Ok(Self(nb))
    }

    fn create_clip(
        &self,
        data: Vec<u8>,
    ) -> Result<Pin<Box<dyn Future<Output = Result<Self::Clip>>>>> {
        let buffer = Uint8Array::from(data.as_slice()).buffer();
        let result = Arc::new(Mutex::new(None));
        let callback = Closure::<dyn Fn(JsValue)>::new({
            let result = Arc::clone(&result);
            move |buffer: JsValue| {
                *result.lock().unwrap() = Some(buffer);
            }
        });
        let _ = self
            .0
            .decode_audio_data_with_success_callback(&buffer, callback.as_ref().unchecked_ref())
            .map_err(js_err)?;
        callback.forget();
        struct DummyFuture(Arc<Mutex<Option<JsValue>>>);
        impl Future for DummyFuture {
            type Output = Result<<WebAudio as Audio>::Clip>;

            fn poll(
                self: Pin<&mut Self>,
                _: &mut std::task::Context<'_>,
            ) -> std::task::Poll<Self::Output> {
                let mut result = self.0.lock().unwrap();
                if let Some(result) = result.take() {
                    log(&result);
                    Poll::Ready(result.try_into().map_err(Error::new))
                } else {
                    Poll::Pending
                }
            }
        }
        Ok(Box::pin(DummyFuture(result)))
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

    fn play(&mut self, clip: &Self::Clip, volume: f64, offset: f64) -> Result<Self::Handle> {
        let gain = self.0.create_gain().map_err(js_err)?;
        gain.gain().set_value(volume as _);
        let node = self.0.create_buffer_source().map_err(js_err)?;
        node.set_buffer(Some(clip));
        node.connect_with_audio_node(&gain).map_err(js_err)?;
        node.connect_with_audio_node(&self.0.destination())
            .map_err(js_err)?;
        node.start_with_when_and_grain_offset(0., offset)
            .map_err(js_err)?;
        Ok(AudioHandle(
            node,
            AudioState::Playing(self.0.current_time() - offset),
            clip.clone(),
            volume,
        ))
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
        *handle = self.play(&handle.2, handle.3, time)?;
        Ok(())
    }

    fn seek_to(&mut self, handle: &mut Self::Handle, position: f64) -> Result<()> {
        handle.0.stop().map_err(js_err)?;
        *handle = self.play(&handle.2, handle.3, position)?;
        Ok(())
    }
}
