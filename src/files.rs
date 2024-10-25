use std::{env, path::PathBuf};

use crate::errors::Error;
use rfd::{AsyncFileDialog, FileHandle};

pub const DEFAULT_LOCATION: &str = ".config/solana/id.json";

pub fn default_keypair_path() -> PathBuf {
    let home_dir = env::var("HOME") // mac users
        .or_else(|_| env::var("USERPROFILE")) // windows users
        .expect("Cannot find home directory");
    let mut path = PathBuf::from(home_dir);
    path.push(DEFAULT_LOCATION);
    path
}

pub enum FileType {
    Keypair,
    Program,
}

pub async fn pick_file(file_type: FileType) -> Result<PathBuf, Error> {
    let handle = AsyncFileDialog::new()
        .set_title("Choose a valid file-type")
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)?;

    let file_extension = match file_type {
        FileType::Keypair => String::from("json"),
        FileType::Program => String::from("so"),
    };

    check_file_extension(handle.clone(), &file_extension)?;

    Ok(handle.path().to_owned())
}

fn check_file_extension(handle: FileHandle, extension: &str) -> Result<(), Error> {
    if handle.path().extension().and_then(|ext| ext.to_str()) != Some(extension) {
        return Err(Error::InvalidFileType);
    }
    Ok(())
}
