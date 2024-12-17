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
use solana_sdk::{native_token::LAMPORTS_PER_SOL, signature::Keypair, signer::Signer};

use crate::{components::copy_to_cliboard_btn, errors::Error, programs::BPrograms};
use crate::{files::default_keypair_path, keypair::load_keypair_from_file, Message};

const RPC_URL: &str = "https://api.devnet.solana.com";

#[derive(Clone)]
pub struct BSettings {
    pub rpc_client: Arc<RpcClient>,
    pub keypair_path: Option<PathBuf>,
    pub program_path: Option<PathBuf>,
    pub keypair: Arc<Keypair>,
    pub balance: Option<u64>,
}

impl Default for BSettings {
    fn default() -> Self {
        let default_keypair_path = default_keypair_path();
        Self {
            rpc_client: Arc::new(RpcClient::new(RPC_URL.to_string())),
            keypair_path: Some(default_keypair_path.to_path_buf()),
            program_path: None,
            keypair: load_keypair_from_file(default_keypair_path).into(),
            balance: None,
        }
    }
}

impl BSettings {
    pub fn view(&self, program_module: &BPrograms) -> Element<'static, Message> {
        let keypair_path = self
            .keypair_path
            .as_deref()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(""));

        let load_keypair = button("Load Keypair").on_press(Message::PickProgramAuthority);

        let keypair = load_keypair_from_file(keypair_path);

        let label = text(format!("Wallet address: ",))
            .size(14)
            .color(color!(0x30cbf2));

        let wallet_address = keypair.pubkey().to_string();
        let copy_btn = copy_to_cliboard_btn(&wallet_address);
        let value = text(wallet_address).size(14);
        let value_with_copy_btn_row = row![value, copy_btn]
            .spacing(10)
            .align_y(iced::Alignment::Center);
        let pubkey_container = column![label, value_with_copy_btn_row];
        let balance_text = match self.balance {
            Some(balance) => column![
                text("SOL Balance: ").color(color!(0x30cbf2)).size(14),
                text(format!(" {:.3}", balance as f32 / LAMPORTS_PER_SOL as f32)).size(14)
            ],
            None => column![text("Loading balance...").size(14)],
        };

        let column_wallet_balance =
            column![pubkey_container, balance_text, load_keypair].spacing(5);

        let rpc_label = text(format!("RPC Client URL: ",))
            .size(14)
            .color(color!(0x30cbf2));

        let rpc_input = text_input("", &self.rpc_client.url())
            .size(14)
            .on_input(Message::RpcClient);

        let set_rpc_client = column![rpc_label, rpc_input];

        let load_program = button("Load Program .so").on_press(Message::PickProgram);

        let program_address = program_module
            .program_account
            .as_ref()
            .map_or(String::from("Undefined program account..."), |v| {
                v.pubkey().to_string()
            });

        let program_label = text(format!("Program pubkey: ",))
            .size(14)
            .color(color!(0x30cbf2));

        let copy_btn = copy_to_cliboard_btn(&program_address);
        let program_text = text(program_address).size(14);
        let program_address_with_copy_btn_row =
            row![program_text, copy_btn].spacing(5).align_y(iced::Alignment::Center);

        let load_program_account =
            button("Load Program Account").on_press(Message::PickProgramAccount);

        let program_account_column = column![
            program_label,
            program_address_with_copy_btn_row,
            load_program_account
        ]
        .spacing(5);

        let program_binaries_column = column![
            text("Program size: ").color(color!(0x30cbf2)).size(14),
            text(format!("{} bytes", program_module.program_bytes.len())).size(14),
            load_program
        ]
        .spacing(5);

        container(
            column![
                column_wallet_balance,
                set_rpc_client,
                program_account_column,
                program_binaries_column
            ]
            .spacing(10),
        )
        .align_x(iced::alignment::Horizontal::Center)
        .into()
    }
}

pub async fn keypair_balance(path: PathBuf, rpc_client: Arc<RpcClient>) -> Result<u64, Error> {
    let keypair = load_keypair_from_file(path);
    rpc_client
        .get_balance(&keypair.pubkey())
        .await
        .map_err(|_| Error::FetchBalanceError)
}
