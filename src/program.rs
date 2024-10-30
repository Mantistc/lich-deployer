use std::{
    fs,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use iced::Command;
use solana_client::{nonblocking::rpc_client::RpcClient, rpc_config::RpcSendTransactionConfig};
use solana_sdk::{
    bpf_loader_upgradeable::{create_buffer, deploy_with_max_program_len, upgrade, write},
    commitment_config::{CommitmentConfig, CommitmentLevel},
    hash::Hash,
    instruction::{Instruction, InstructionError},
    message::{v0::Message as TransactionMessage, MessageHeader, VersionedMessage},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::{Transaction, TransactionError, VersionedTransaction},
};
use solana_transaction_status::UiTransactionEncoding;
use tokio::time;

use crate::{errors::Error, BlichDeployer, Message};

const BYTES_PER_CHUNK: usize = 1011;

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

    loop {
        if offset == program_bytes.len() {
            break;
        }
        let chunk = program_bytes[offset..offset + BYTES_PER_CHUNK].to_vec();
        let write_ix = write(&buffer_address, &authority.pubkey(), offset as u32, chunk);
        offset += BYTES_PER_CHUNK;
        let mut tx = Transaction::new_with_payer(&[write_ix], Some(&authority.pubkey()));
        tx.sign(&[&authority], recent_blockhash);
        transactions.push(tx);
    }
    transactions
}

pub async fn process_transactions(
    program_path: PathBuf,
    state: Arc<BlichDeployer>,
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
        .get_minimum_balance_for_rent_exemption(program_bytes.len() + 45)
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
            let is_buff_created = confirmation.satisfies_commitment(CommitmentConfig::finalized());
            if is_buff_created {
                break;
            }
        }
        time::sleep(Duration::from_millis(500)).await
    }

    Ok(buffer_account.pubkey().to_string())
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
            println!("first 10 bytes: {:?}", &bytes[..10]);
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
