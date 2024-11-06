use iced::subscription::channel;
use iced::{
    futures::{channel, stream},
    Subscription,
};
use solana_client::{
    nonblocking::rpc_client::RpcClient, rpc_client::SerializableTransaction,
    rpc_config::RpcSendTransactionConfig,
};
use solana_sdk::{
    bpf_loader_upgradeable::{create_buffer, deploy_with_max_program_len, upgrade, write},
    commitment_config::{CommitmentConfig, CommitmentLevel},
    hash::Hash,
    instruction::{Instruction, InstructionError},
    pubkey::Pubkey,
    signature::{Keypair, Signature},
    signer::Signer,
    transaction::{Transaction, TransactionError, VersionedTransaction},
};
use solana_transaction_status::UiTransactionEncoding;
use std::{fs, path::PathBuf, sync::Arc, time::Duration};
use tokio::{
    spawn,
    sync::{mpsc, Mutex},
    time,
};

use crate::{errors::Error, BlichDeployer, Message};

const BYTES_PER_CHUNK: usize = 1011;
const PROGRAM_EXTRA_SPACE: usize = 45;

pub fn create_buffer_account(
    authority: &Keypair,
    lamports: u64,
    program_bytes: &Vec<u8>,
    recent_blockhash: Hash,
) -> Result<(Keypair, Transaction), InstructionError> {
    let buffer_account = Keypair::new();
    println!("lamports: {}, bytes: {:?}", lamports, program_bytes.len());
    let create_buffer_ix = create_buffer(
        &authority.pubkey(),
        &buffer_account.pubkey(),
        &authority.pubkey(),
        lamports,
        program_bytes.len(),
    )?;
    let mut tx = Transaction::new_with_payer(&create_buffer_ix, Some(&authority.pubkey()));
    tx.sign(&[&authority, &buffer_account], recent_blockhash);

    Ok((buffer_account, tx))
}

pub fn write_data(
    buffer_address: &Pubkey,
    program_bytes: &Vec<u8>,
    authority: &Keypair,
    recent_blockhash: Hash,
) -> Vec<Transaction> {
    let mut offset = 0;
    let mut transactions = Vec::new();

    while offset < program_bytes.len() {
        let end = (offset + BYTES_PER_CHUNK).min(program_bytes.len());

        let chunk = program_bytes[offset..end].to_vec();

        let write_ix = write(&buffer_address, &authority.pubkey(), offset as u32, chunk);

        offset += BYTES_PER_CHUNK;

        let mut tx = Transaction::new_with_payer(&[write_ix], Some(&authority.pubkey()));
        tx.sign(&[&authority], recent_blockhash);

        transactions.push(tx)
    }

    transactions
}

pub async fn process_transactions(
    program_path: PathBuf,
    state: Arc<BlichDeployer>,
    progress_sender: mpsc::Sender<(usize, usize)>,
) -> Result<String, Error> {
    let program_bytes =
        get_program_bytes(program_path.to_str().unwrap_or("")).unwrap_or(Vec::new());

    let authority = state.keypair.clone();
    let rpc_client = state.rpc_client.clone();

    let blockhash_result = rpc_client
        .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
        .await;

    let recent_blockhash = if let Ok((blockhash_info, _)) = blockhash_result {
        blockhash_info
    } else {
        return Err(Error::FetchBlockhashError);
    };

    let lamports = rpc_client
        .get_minimum_balance_for_rent_exemption(program_bytes.len() + PROGRAM_EXTRA_SPACE)
        .await
        .unwrap_or(0);

    if lamports == 0 {
        return Err(Error::InvalidAmount);
    }
    let (buffer_account, buffer_acc_init_tx) =
        create_buffer_account(&authority, lamports, &program_bytes, recent_blockhash)
            .map_err(|e| Error::InstructionError(e))?;

    let send_cfg = RpcSendTransactionConfig {
        skip_preflight: false,
        preflight_commitment: Some(CommitmentLevel::Confirmed),
        encoding: Some(UiTransactionEncoding::Base64),
        max_retries: Some(3),
        min_context_slot: None,
    };

    let signature = rpc_client
        .send_transaction_with_config(&buffer_acc_init_tx, send_cfg)
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
                _ => return Err(Error::FetchBalanceError),
            };
        }
        time::sleep(Duration::from_millis(500)).await
    }

    let blockhash_result2 = rpc_client
        .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
        .await;

    let updated_blockhash = if let Ok((blockhash_info, _)) = blockhash_result2 {
        blockhash_info
    } else {
        return Err(Error::FetchBlockhashError);
    };

    let mut write_data_txs = write_data(
        &buffer_account.pubkey(),
        &program_bytes,
        &authority,
        updated_blockhash,
    );

    let mut tx_sent = 0;

    // send all tx

    for transaction in write_data_txs.clone() {
        tx_sent += 1;
        let _ = progress_sender.send((tx_sent, write_data_txs.len())).await;
        let client = rpc_client.clone();
        let config = send_cfg.clone();
        let tx = transaction.clone();
        spawn(async move {
            let _ = client.send_transaction_with_config(&tx, config).await;
        });
        // time::sleep(Duration::from_millis(25)).await
    }

    let tx_signatures: Vec<Signature> = write_data_txs
        .clone()
        .into_iter()
        .map(|tx| *tx.get_signature())
        .take(200)
        .collect();

    loop {
        tx_sent = 0;
        let mut tx_to_retry: Vec<Signature> = Vec::new();

        let status_vec = &rpc_client
            .get_signature_statuses(&tx_signatures)
            .await
            .map_err(|e| Error::RpcError(e))?
            .value;

        for (i, status) in status_vec.iter().enumerate() {
            if let Some(confirmation) = status {
                if confirmation.err.is_some() {
                    tx_to_retry.push(tx_signatures[i]);
                }
            } else {
                tx_to_retry.push(tx_signatures[i]);
            }
        }
        let save_last_value = write_data_txs.clone();
        write_data_txs = Vec::new();

        for signature in &tx_to_retry {
            let mut tx_from_signature = None;

            for tx in save_last_value.clone() {
                let write_tx_signature = tx.get_signature();
                if *write_tx_signature == *signature {
                    println!("equal?: {:?}", *write_tx_signature == *signature);
                    tx_from_signature = Some(tx);
                    break;
                }
            }
            if let Some(tx) = tx_from_signature {
                write_data_txs.push(tx);
            }
        }
        println!(
            "tx to retry: {}, tx array len: {}",
            tx_to_retry.clone().len(),
            write_data_txs.len()
        );

        for transaction in write_data_txs.clone() {
            println!("tx sent: {}, total: {}", tx_sent, write_data_txs.len());
            tx_sent += 1;
            let _ = progress_sender.send((tx_sent, write_data_txs.len())).await;
            let client = rpc_client.clone();
            let config = send_cfg.clone();
            let tx = transaction.clone();
            spawn(async move {
                let _ = client.send_transaction_with_config(&tx, config).await;
            });
            time::sleep(Duration::from_millis(25)).await
        }
        if tx_to_retry.len() == 0 {
            break;
        }
    }
    Ok(buffer_account.pubkey().to_string())
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Progress {
    Sending { sent: usize, total: usize },
    Completed,
    Idle,
}

pub fn progress_subscription(
    progress_receiver: Arc<Mutex<Option<mpsc::Receiver<(usize, usize)>>>>,
) -> Subscription<Progress> {
    iced::subscription::unfold("progress_subscription", (), move |_| {
        let progress_receiver = Arc::clone(&progress_receiver);
        async move {
            let mut receiver = progress_receiver.lock().await;
            if let Some(ref mut rx) = *receiver {
                if let Some((sent, total)) = rx.recv().await {
                    (Progress::Sending { sent, total }, ())
                } else {
                    (Progress::Completed, ())
                }
            } else {
                (Progress::Idle, ())
            }
        }
    })
}

pub fn deploy_program(
    authority: Keypair,
    program_keypair: Keypair,
    buffer_address: &Pubkey,
    program_bytes: &Vec<u8>,
    program_lamports: u64,
) -> Result<Vec<Instruction>, InstructionError> {
    let payer_address = &authority.pubkey();
    let program_address = &program_keypair.pubkey();
    let len = program_bytes.len();

    let deploy_program = deploy_with_max_program_len(
        payer_address,
        program_address,
        buffer_address,
        payer_address,
        program_lamports,
        len,
    );
    deploy_program
}

pub fn upgrade_program(
    program_keypair: Keypair,
    buffer_address: &Pubkey,
    program_bytes: &Vec<u8>,
    authority: Keypair,
) -> Instruction {
    let authority_pubkey = &authority.pubkey();
    let program_address = &program_keypair.pubkey();
    let upgrade_program = upgrade(
        program_address,
        buffer_address,
        authority_pubkey,
        authority_pubkey,
    );
    upgrade_program
}

pub fn get_program_bytes(program_path: &str) -> Result<Vec<u8>, InstructionError> {
    match fs::read(program_path) {
        Ok(bytes) => {
            println!("bytes len: {}", bytes.len());
            if bytes.len() == 0 {
                return Err(InstructionError::ProgramEnvironmentSetupFailure);
            }
            Ok(bytes)
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            return Err(InstructionError::InvalidAccountData);
        }
    }
}
