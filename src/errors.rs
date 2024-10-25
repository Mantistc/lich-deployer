#[derive(Debug, Clone)]
pub enum Error {
    DialogClosed,
    FetchBalanceError,
    InvalidFileType,
    FetchBlockhashError,
    TransactionError,
    InvalidAmount,
    InvalidPubKeyLen,
    InsufficientBalance
}