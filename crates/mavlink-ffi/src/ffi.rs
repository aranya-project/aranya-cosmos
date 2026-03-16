use aranya_crypto::Engine;
use aranya_policy_vm::{ffi::ffi, CommandContext};

use crate::{error::DataMissing, Handle, Shared};

/// Implements the MAVLink module.
#[derive(Debug)]
pub struct Ffi {
    shared: Shared,
}

impl Ffi {
    pub fn new() -> (Self, Handle) {
        let shared = Shared::default();
        let handle = Handle {
            shared: shared.clone(),
        };
        let ffi = Self { shared };
        (ffi, handle)
    }
}

#[ffi(module = "mavlink")]
impl Ffi {
    #[ffi_export(def = r#"function get_sender_sys_id() int"#)]
    fn get_sender_sys_id<E: Engine>(
        &self,
        _ctx: &CommandContext,
        _eng: &E,
    ) -> Result<i64, DataMissing> {
        let guard = self.shared.lock().expect("poisoned");
        let data = guard.as_ref().ok_or(DataMissing)?;
        Ok(data.sender_sys_id.into())
    }

    #[ffi_export(def = r#"function get_target_sys_id() int"#)]
    fn get_target_sys_id<E: Engine>(
        &self,
        _ctx: &CommandContext,
        _eng: &E,
    ) -> Result<i64, DataMissing> {
        let guard = self.shared.lock().expect("poisoned");
        let data = guard.as_ref().ok_or(DataMissing)?;
        Ok(data.target_sys_id.into())
    }

    #[ffi_export(def = r#"function get_task_id() int"#)]
    fn get_task_id<E: Engine>(&self, _ctx: &CommandContext, _eng: &E) -> Result<i64, DataMissing> {
        let guard = self.shared.lock().expect("poisoned");
        let data = guard.as_ref().ok_or(DataMissing)?;
        Ok(data.task_id.into())
    }
}
