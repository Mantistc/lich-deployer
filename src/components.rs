use std::path::PathBuf;

use crate::{errors::Error, keypair::load_keypair_from_file, Message};
use iced::{
    color,
    widget::{button, column, progress_bar, row, text, text_input},
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

pub fn buffer_address(buffer: &str) -> Element<'static, Message> {
    let label = text(format!("Buffer Address: ",))
        .size(14)
        .style(color!(0x30cbf2));

    let value = text(buffer).size(14);

    let container = column![label, value];
    container.into()
}

pub fn tx_progress(current: usize, total: usize) -> Element<'static, Message> {
    let label = text(format!("Transaction progress: ",))
        .size(14)
        .style(color!(0x30cbf2));
    let values = text(format!("{}/{}", current, total)).size(14);
    let progress_bar = progress_bar(0.0..=total as f32, current as f32);
    let counter = row![label, values];
    let container = column![counter, progress_bar];
    container.into()
}

pub fn load_program_btn() -> Element<'static, Message> {
    let load_btn = button("Load Program Binaries").on_press(Message::PickProgram);
    load_btn.into()
}

pub fn deploy_program_btn(program_path: PathBuf) -> Element<'static, Message> {
    let load_btn = button("Deploy").on_press(Message::DeployProgram(program_path));
    load_btn.into()
}

pub fn handle_rpc_url(url: &str) -> Element<'static, Message> {
    let label = text(format!("RPC Client URL: ",))
        .size(14)
        .style(color!(0x30cbf2));

    let value = text_input("", url).size(14).on_input(Message::RpcClient);

    let container = column![label, value];
    container.into()
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
