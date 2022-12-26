use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
};

use tokio::task::JoinError;

pub struct Task<T: Send + 'static>(Option<Pin<Box<dyn Future<Output = Result<(), JoinError>> + Send>>>, Arc<Mutex<Option<T>>>);

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
