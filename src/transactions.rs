use std::time::Duration;

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_client::rpc_config::RpcSendTransactionConfig;
use solana_sdk::{signature::Signature, transaction::Transaction};
use tokio::time;

use crate::errors::Error;

// this send the tx and verify its confimation
// if there's any error on the tx status, the loop will break.
pub async fn send_tx_and_verify_status(
    rpc_client: &RpcClient,
    tx: &Transaction,
    rpc_config: RpcSendTransactionConfig,
) -> Result<Signature, Error> {
    let signature = rpc_client
        .send_transaction_with_config(tx, rpc_config)
        .await
        .map_err(|e| Error::RpcError(e))?;

    loop {
        let status = &rpc_client
            .get_signature_statuses(&[signature])
            .await
            .map_err(|e| Error::RpcError(e))?
            .value[0];

        if let Some(confirmation) = status {
            if let Some(error) = &confirmation.err {
                return Err(Error::TransactionError(error.clone()));
            }
            let confirm_status = confirmation.confirmation_status();
            match confirm_status {
                solana_transaction_status::TransactionConfirmationStatus::Processed => continue,
                solana_transaction_status::TransactionConfirmationStatus::Confirmed
                | solana_transaction_status::TransactionConfirmationStatus::Finalized => break,
                _ => return Err(Error::TransactionConfirmationStatusFailed),
            };
        }
        time::sleep(Duration::from_millis(500)).await
    }
    Ok(signature)
}
