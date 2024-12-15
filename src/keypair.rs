use std::path::PathBuf;
use solana_sdk::signature::{read_keypair_file, Keypair};

pub fn load_keypair_from_file(path: PathBuf) -> Keypair {
    let keypair = read_keypair_file(path).unwrap_or(Keypair::new());
    keypair
}
