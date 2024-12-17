use solana_client::client_error::ClientError;
use solana_sdk::{instruction::InstructionError, transaction::TransactionError};

#[derive(Debug)]
pub enum Error {
    DialogClosed,
    FetchBalanceError,
    InvalidFileType,
    TransactionError(TransactionError),
    RpcError(ClientError),
    InstructionError(InstructionError),
    InvalidProgramLen,
    UnexpectedError,
    ProgramAccountNotLoaded
}

impl From<TransactionError> for Error {
    fn from(error: TransactionError) -> Self {
        Error::TransactionError(error)
    }
}

impl From<InstructionError> for Error {
    fn from(error: InstructionError) -> Self {
        Error::InstructionError(error)
    }
}

impl Clone for Error {
    fn clone(&self) -> Self {
        match self {
            Error::DialogClosed => Error::DialogClosed,
            Error::FetchBalanceError => Error::FetchBalanceError,
            Error::InvalidFileType => Error::InvalidFileType,
            Error::TransactionError(e) => Error::TransactionError(e.clone()),
            Error::InstructionError(e) => Error::InstructionError(e.clone()),
            Error::RpcError(e) => Error::RpcError(ClientError {
                request: e.request.clone(),
                kind: solana_client::client_error::ClientErrorKind::Custom(String::from(
                    e.to_string(),
                )),
            }),
            Error::InvalidProgramLen => Error::InvalidProgramLen,
            Error::UnexpectedError => Error::UnexpectedError,
            Error::ProgramAccountNotLoaded => Error::ProgramAccountNotLoaded,
        }
    }
}
