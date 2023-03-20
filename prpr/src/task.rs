use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

#[cfg(not(target_arch = "wasm32"))]
pub struct Task<T: Send + 'static>(Option<Pin<Box<dyn Future<Output = Result<(), tokio::task::JoinError>> + Send>>>, Arc<Mutex<Option<T>>>);
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + 'static> Task<T> {
    pub fn new(future: impl Future<Output = T> + Send + 'static) -> Self {
        let arc = Arc::new(Mutex::new(None));
        Self(
            Some({
                let arc = Arc::clone(&arc);
                Box::pin(tokio::spawn(async move {
                    let result = future.await;
                    *arc.lock().unwrap() = Some(result);
                }))
            }),
            arc,
        )
    }

    pub fn pending() -> Self {
        Self::new(std::future::pending())
    }

    pub fn ok(&self) -> bool {
        self.1.lock().unwrap().is_some()
    }

    pub fn take(&mut self) -> Option<T> {
        self.1.lock().unwrap().take()
    }
}

#[cfg(target_arch = "wasm32")]
pub struct Task<T>(std::marker::PhantomData<T>);
#[cfg(target_arch = "wasm32")]
impl<T> Task<T> {
    pub fn take(&mut self) -> Option<T> {
        unimplemented!()
    }
}
