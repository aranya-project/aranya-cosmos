#[derive(Debug)]
pub struct DataMissing;

#[derive(Debug)]
pub struct DataPresent;

impl From<DataMissing> for aranya_policy_vm::MachineError {
    fn from(err: DataMissing) -> Self {
        aranya_policy_vm::MachineError::new(aranya_policy_vm::MachineErrorType::Unknown(format!(
            "{err:?}"
        )))
    }
}
