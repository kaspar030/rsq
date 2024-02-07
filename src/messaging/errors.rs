use thiserror::Error;

#[derive(Debug, Error)]
pub enum TxError {
    #[error("no subscriber")]
    NoSubscriber,
}
