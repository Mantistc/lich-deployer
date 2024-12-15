use solana_sdk::{
    bpf_loader_upgradeable::{
        create_buffer, deploy_with_max_program_len, set_buffer_authority, upgrade, write,
    },
    hash::Hash,
    instruction::{Instruction, InstructionError},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};

use crate::programs::{get_vec_with_batched_data, BYTES_PER_CHUNK};

pub fn create_buffer_account(
    buffer_account: &Keypair,
    authority: &Keypair,
    lamports: u64,
    program_bytes: &Vec<u8>,
    recent_blockhash: Hash,
) -> Result<Transaction, InstructionError> {
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

    Ok(tx)
}

pub fn write_data(
    buffer_address: &Pubkey,
    program_bytes: &Vec<u8>,
    authority: &Keypair,
    recent_blockhash: Hash,
) -> Vec<Transaction> {
    let mut transactions = Vec::new();
    let write_data_batches = get_vec_with_batched_data(BYTES_PER_CHUNK, program_bytes);
    for (index, data) in write_data_batches.into_iter().enumerate() {
        let write_ix = write(
            &buffer_address,
            &authority.pubkey(),
            index as u32 * BYTES_PER_CHUNK as u32,
            data,
        );
        let mut tx = Transaction::new_with_payer(&[write_ix], Some(&authority.pubkey()));
        tx.sign(&[&authority], recent_blockhash);
        transactions.push(tx)
    }
    transactions
}

// TODO: implements set a new buffer authority
// this works if you want to set your Squad as authority
pub fn _set_new_buffer_auth(
    buffer_address: &Pubkey,
    authority: &Keypair,
    recent_blockhash: Hash,
    new_authority: &Pubkey,
) -> Transaction {
    let set_new_auth_ix = set_buffer_authority(buffer_address, &authority.pubkey(), new_authority);
    let mut tx = Transaction::new_with_payer(&[set_new_auth_ix], Some(&authority.pubkey()));
    tx.sign(&[&authority], recent_blockhash);
    tx
}

// TODO: implements deploy instructions
pub fn _deploy_program(
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

// TODO: implements upgrade program instructions
pub fn _upgrade_program(
    program_keypair: Keypair,
    buffer_address: &Pubkey,
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
