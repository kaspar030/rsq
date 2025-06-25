use thiserror::Error;

#[derive(Debug, Error)]
pub enum TxError {
    #[error("invalid channel id")]
    InvalidChannel,
    #[error("no subscriber")]
    NoSubscriber,
}
