use std::{path::PathBuf, sync::Arc};

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    signature::{read_keypair_file, Keypair},
    signer::Signer,
};

use crate::errors::Error;

pub fn load_keypair_from_file(path: PathBuf) -> Keypair {
    let keypair = read_keypair_file(path).unwrap_or(Keypair::new());
    keypair
}

pub async fn keypair_balance(path: PathBuf, rpc_client: Arc<RpcClient>) -> Result<u64, Error> {
    let keypair = load_keypair_from_file(path);
    rpc_client
        .get_balance(&keypair.pubkey())
        .await
        .map_err(|_| Error::FetchBalanceError)
}
