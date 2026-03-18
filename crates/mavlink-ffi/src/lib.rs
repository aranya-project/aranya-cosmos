//! The MAVLink FFI module.

mod error;
mod ffi;

use std::sync::{Arc, Mutex};

pub use aranya_daemon_api::MavData;

pub use crate::{
    error::{DataMissing, DataPresent},
    ffi::Ffi,
};

#[derive(Debug)]
pub struct Handle {
    shared: Shared,
}

impl Handle {
    pub fn set(&mut self, data: MavData) -> Result<(), DataPresent> {
        let mut guard = self.shared.lock().expect("poisoned");
        if guard.is_some() {
            return Err(DataPresent);
        }
        *guard = Some(data);
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), DataMissing> {
        let mut guard = self.shared.lock().expect("poisoned");
        if guard.is_some() {
            return Err(DataMissing);
        }
        *guard = None;
        Ok(())
    }
}

type Shared = Arc<Mutex<Option<MavData>>>;
