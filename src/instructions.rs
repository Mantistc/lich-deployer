use bincode::serialized_size;
use solana_sdk::{
    bpf_loader_upgradeable::{
        create_buffer, deploy_with_max_program_len, set_buffer_authority, upgrade, write,
    },
    compute_budget::ComputeBudgetInstruction,
    hash::Hash,
    instruction::{Instruction, InstructionError},
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};

use crate::{programs::get_vec_with_batched_data, settings::LSettings};

pub fn get_priority_fees_ixs(unit_limit: u32, unit_price: u64) -> [Instruction; 2] {
    let comput_unit_limit_ix = ComputeBudgetInstruction::set_compute_unit_limit(unit_limit);
    let comput_unit_price_ix = ComputeBudgetInstruction::set_compute_unit_price(unit_price);
    [comput_unit_limit_ix, comput_unit_price_ix]
}

pub fn create_buffer_account(
    buffer_account: &Keypair,
    authority: &Keypair,
    lamports: u64,
    program_bytes: &Vec<u8>,
    recent_blockhash: Hash,
    settings: &LSettings,
) -> Result<Transaction, InstructionError> {
    println!("lamports: {}, bytes: {:?}", lamports, program_bytes.len());
    let mut create_buffer_ix = create_buffer(
        &authority.pubkey(),
        &buffer_account.pubkey(),
        &authority.pubkey(),
        lamports,
        program_bytes.len(),
    )?;
    let priority_ixs = get_priority_fees_ixs(settings.unit_limit, settings.unit_price);
    create_buffer_ix.splice(0..0, priority_ixs);
    let mut tx = Transaction::new_with_payer(&create_buffer_ix, Some(&authority.pubkey()));
    tx.sign(&[&authority, &buffer_account], recent_blockhash);

    Ok(tx)
}

pub fn write_data(
    buffer_address: &Pubkey,
    program_bytes: &Vec<u8>,
    authority: &Keypair,
    recent_blockhash: Hash,
    bytes_per_chunk: usize,
    settings: &LSettings,
) -> Vec<Transaction> {
    let mut transactions = Vec::new();
    let write_data_batches = get_vec_with_batched_data(bytes_per_chunk, program_bytes);
    let priority_ixs = get_priority_fees_ixs(settings.unit_limit, settings.unit_price);
    for (index, data) in write_data_batches.into_iter().enumerate() {
        let mut ixs = Vec::new();
        let write_ix = write(
            &buffer_address,
            &authority.pubkey(),
            index as u32 * bytes_per_chunk as u32,
            data,
        );

        ixs.extend_from_slice(&priority_ixs);
        ixs.push(write_ix.clone());
        let mut tx = Transaction::new_with_payer(&ixs, Some(&authority.pubkey()));
        tx.sign(&[&authority], recent_blockhash);
        let size =serialized_size(&tx).unwrap() as usize;
        let ix_write_size =serialized_size(&write_ix).unwrap() as usize;
        println!("tx size: {}, tx_chunk: {}, write_ix_size: {}", size, bytes_per_chunk, ix_write_size);
        transactions.push(tx)
    }
    transactions
}

// TODO: implements set a new buffer authority
// this will be useful if you want to use your squad authority to upgrade a program using this buffer account
pub fn set_new_buffer_auth(
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

pub fn deploy_program(
    authority: &Keypair,
    program_keypair: &Keypair,
    buffer_address: &Pubkey,
    program_bytes: &Vec<u8>,
    program_lamports: u64,
    recent_blockhash: Hash,
) -> Result<Transaction, InstructionError> {
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
    )?;

    let mut tx = Transaction::new_with_payer(&deploy_program, Some(&authority.pubkey()));
    tx.sign(&[&authority, &program_keypair], recent_blockhash);
    Ok(tx)
}

pub fn upgrade_program(
    program_keypair: &Keypair,
    buffer_address: &Pubkey,
    authority: &Keypair,
    recent_blockhash: Hash,
) -> Transaction {
    let authority_pubkey = &authority.pubkey();
    let program_address = &program_keypair.pubkey();
    let upgrade_program_ix = upgrade(
        program_address,
        buffer_address,
        authority_pubkey,
        authority_pubkey,
    );
    let mut tx = Transaction::new_with_payer(&[upgrade_program_ix], Some(&authority.pubkey()));
    tx.sign(&[&authority], recent_blockhash);
    tx
}
