use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use iced::{
    color,
    widget::{button, column, container, row, text, text_input},
    Element,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::{signature::Keypair, signer::Signer};

use crate::{files::default_keypair_path, keypair::load_keypair_from_file, Message};

const RPC_URL: &str = "https://api.devnet.solana.com";

#[derive(Clone)]
pub struct BSettings {
    pub rpc_client: Arc<RpcClient>,
    pub keypair_path: Option<PathBuf>,
    pub program_path: Option<PathBuf>,
    pub keypair: Arc<Keypair>,
}

impl Default for BSettings {
    fn default() -> Self {
        let default_keypair_path = default_keypair_path();
        Self {
            rpc_client: Arc::new(RpcClient::new(RPC_URL.to_string())),
            keypair_path: Some(default_keypair_path.to_path_buf()),
            program_path: None,
            keypair: load_keypair_from_file(default_keypair_path).into(),
        }
    }
}

impl BSettings {
    pub fn view(&self) -> Element<'static, Message> {
        let keypair_path = self
            .keypair_path
            .as_deref()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(""));

        let load_keypair = button("Load Keypair").on_press(Message::PickProgramAuthority);
        let load_program = button("Load Program Binaries").on_press(Message::PickProgram);

        let keypair = load_keypair_from_file(keypair_path);

        let label = text(format!("Wallet address: ",))
            .size(14)
            .style(color!(0x30cbf2));

        let value = text(keypair.pubkey().to_string()).size(14);

        let pubkey_container = column![label, value];

        let rpc_label = text(format!("RPC Client URL: ",))
            .size(14)
            .style(color!(0x30cbf2));

        let rpc_input = text_input("", &self.rpc_client.url())
            .size(14)
            .on_input(Message::RpcClient);

        let set_rpc_client = column![rpc_label, rpc_input];
        let btns = row![load_keypair, load_program].spacing(10);

        container(column![btns, pubkey_container, set_rpc_client,].spacing(10))
            .align_x(iced::alignment::Horizontal::Center)
            .into()
    }
}
