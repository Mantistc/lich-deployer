use components::{
    deploy_program_btn, error, keypair_pbkey_address, load_keypair_btn, load_program_btn,
};
use iced::{
    executor,
    widget::{column, container},
    Application, Command, Element, Renderer, Settings, Theme,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use std::{io::{BufRead, BufReader}, sync::Arc};
use std::path::{Path, PathBuf};
use std::process::{Command as StdCommand, Stdio};
use std::thread;
mod components;
mod errors;
mod files;
mod keypair;
mod program;

use errors::Error;
use files::{default_keypair_path, pick_file, FileType, DEFAULT_LOCATION};
use keypair::load_keypair_from_file;
use solana_sdk::signature::Keypair;

fn main() -> iced::Result {
    BlichDeployer::run(Settings::default())
}

const RPC_URL: &str = "https://api.devnet.solana.com";

struct BlichDeployer {
    pub keypair_path: Option<PathBuf>,
    pub keypair: Keypair,
    pub program_path: Option<PathBuf>,
    pub rpc_client: Arc<RpcClient>,
    pub error: Option<Error>,
}

#[derive(Debug, Clone)]
enum Message {
    PickProgramAuthority,
    LoadProgramAuthority(Result<PathBuf, Error>),
    PickProgram,
    LoadProgram(Result<PathBuf, Error>),
    DeployProgram(PathBuf, PathBuf),
}

impl Application for BlichDeployer {
    type Executor = executor::Default;

    type Message = Message;

    type Theme = Theme;

    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (
            Self {
                keypair_path: None,
                program_path: None,
                rpc_client: Arc::new(RpcClient::new(RPC_URL.to_string())),
                keypair: Keypair::new(),
                error: None,
            },
            Command::perform(
                async { Ok(default_keypair_path()) },
                Message::LoadProgramAuthority,
            ),
        )
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::PickProgramAuthority => {
                Command::perform(pick_file(FileType::Keypair), Message::LoadProgramAuthority)
            }
            Message::LoadProgramAuthority(Ok(path)) => {
                self.keypair_path = Some(path.to_path_buf());
                self.keypair = load_keypair_from_file(path.to_path_buf()).into();
                Command::none()
            }
            Message::LoadProgramAuthority(Err(err)) => {
                self.error = Some(err);
                Command::none()
            }
            Message::PickProgram => {
                Command::perform(pick_file(FileType::Program), Message::LoadProgram)
            }
            Message::LoadProgram(Ok(path)) => {
                self.program_path = Some(path.to_path_buf());
                Command::none()
            }
            Message::LoadProgram(Err(err)) => {
                self.error = Some(err);
                Command::none()
            }
            Message::DeployProgram(program_path, keypair_path) => {
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Self::Message, Renderer<Self::Theme>> {
        let keypair_path = self
            .keypair_path
            .as_deref()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(""));

        let program_path = self
            .program_path
            .as_deref()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(""));

        let load_keypair_btn = load_keypair_btn();
        let display_pubkey = keypair_pbkey_address(keypair_path.to_path_buf());
        let display_error = error(&self.error);
        let load_program_btn = load_program_btn();
        let deploy_program_btn = deploy_program_btn(program_path, keypair_path);

        container(
            column![
                display_pubkey,
                load_keypair_btn,
                load_program_btn,
                display_error,
                deploy_program_btn
            ]
            .spacing(14),
        )
        .padding(30)
        .into()
    }

    fn title(&self) -> String {
        String::from("Blich Deployer Application")
    }

    fn theme(&self) -> Theme {
        Theme::Dark
    }
}
