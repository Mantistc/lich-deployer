use std::path::PathBuf;

use crate::{errors::Error, keypair::load_keypair_from_file, Message};
use iced::{
    color,
    widget::{button, column, text},
    Element,
};
use solana_sdk::signer::Signer;

pub fn load_keypair_btn() -> Element<'static, Message> {
    let load_btn = button("Load Keypair").on_press(Message::PickProgramAuthority);
    load_btn.into()
}

pub fn keypair_pbkey_address(file_path: PathBuf) -> Element<'static, Message> {
    let keypair = load_keypair_from_file(file_path);

    let label = text(format!("Wallet address: ",))
        .size(14)
        .style(color!(0x30cbf2));

    let value = text(keypair.pubkey().to_string()).size(14);

    let pubkey_container = column![label, value];
    pubkey_container.into()
}

pub fn load_program_btn() -> Element<'static, Message> {
    let load_btn = button("Load Program Binaries").on_press(Message::PickProgram);
    load_btn.into()
}

pub fn deploy_program_btn(
    program_path: PathBuf,
    keypair_path: PathBuf,
) -> Element<'static, Message> {
    let load_btn = button("Load Program Binaries")
        .on_press(Message::DeployProgram(program_path, keypair_path));
    load_btn.into()
}

pub fn error(error: &Option<Error>) -> Element<'static, Message> {
    let error_message = if let Some(ref error) = error {
        text(format!("Error: {:?}", error))
            .size(14)
            .style(color!(0xFF0000))
    } else {
        text("").size(1)
    };
    error_message.into()
}
