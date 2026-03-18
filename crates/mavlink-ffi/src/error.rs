#[derive(Debug, thiserror::Error)]
#[error("data was missing")]
pub struct DataMissing;

#[derive(Debug, thiserror::Error)]
#[error("data was present")]
pub struct DataPresent;

impl From<DataMissing> for aranya_policy_vm::MachineError {
    fn from(err: DataMissing) -> Self {
        aranya_policy_vm::MachineError::new(aranya_policy_vm::MachineErrorType::Unknown(format!(
            "{err:?}"
        )))
    }
}
