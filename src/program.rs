use std::{fs, sync::Arc};

use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{
    bpf_loader_upgradeable::{create_buffer, write, deploy_with_max_program_len, upgrade},
    instruction::{Instruction, InstructionError},
    signature::Keypair,
    signer::Signer,
};

use crate::errors::Error;

#[derive(Debug)]
pub struct DeployValues {
    buffer_account: Keypair,
    program_bytes: Vec<u8>,
}

pub async fn create_buffer_account(
    authority: Keypair,
    program_path: &str,
    rpc_client: Arc<RpcClient>,
) -> Result<(DeployValues, Vec<Instruction>), InstructionError> {
    let buffer_account = Keypair::new();
    let program_bytes = get_program_bytes(program_path).unwrap_or(Vec::new());

    if program_bytes.len() == 0 {
        return Err(InstructionError::ProgramEnvironmentSetupFailure);
    }

    let lamports_required = rpc_client
        .get_minimum_balance_for_rent_exemption(program_bytes.len())
        .await
        .unwrap_or(0);

    if lamports_required == 0 {
        return Err(InstructionError::AccountDataTooSmall);
    }

    let create_buffer_ix = create_buffer(
        &authority.pubkey(),
        &buffer_account.pubkey(),
        &authority.pubkey(),
        lamports_required,
        program_bytes.len(),
    )?;

    Ok((
        DeployValues {
            buffer_account,
            program_bytes,
        },
        create_buffer_ix,
    ))
}

pub fn write_data_into_buffer(dp_values: DeployValues, authority: Keypair) -> Instruction {
    let buffer_address = dp_values.buffer_account;
    let bytes = dp_values.program_bytes;
    let offset = 0;

    let write_data_ix = write(&buffer_address.pubkey(), &authority.pubkey(), offset, bytes);
    write_data_ix
}

pub fn deploy_program() {
    // let deploy_program = deploy_with_max_program_len(payer_address, program_address, buffer_address, upgrade_authority_address, program_lamports, max_data_len);
}

pub fn get_program_bytes(program_path: &str) -> Result<Vec<u8>, Error> {
    match fs::read(program_path) {
        Ok(bytes) => {
            println!("bytes len: {}", bytes.len());
            println!("first 10 bytes: {:?}", &bytes[..10]);
            Ok(bytes)
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            return Err(Error::FetchBalanceError);
        }
    }
}
