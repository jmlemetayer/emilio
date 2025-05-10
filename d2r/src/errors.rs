use thiserror::Error;

use crate::tracker::OsEvent;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    InputOutput(#[from] std::io::Error),
    #[error(transparent)]
    TokioSyncMpscSend(#[from] tokio::sync::mpsc::error::SendError<OsEvent>),
    #[error(transparent)]
    WMI(#[from] wmi::WMIError),
}

pub type Result<T> = std::result::Result<T, Error>;
