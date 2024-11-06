use components::{
    buffer_address, deploy_program_btn, error, handle_rpc_url, keypair_pbkey_address,
    load_keypair_btn, load_program_btn, tx_progress,
};
use iced::{
    executor,
    widget::{column, container},
    Application, Command, Element, Renderer, Settings, Subscription, Theme,
};
use program::{process_transactions, progress_subscription, Progress};
use solana_client::nonblocking::rpc_client::RpcClient;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
mod components;
mod errors;
mod files;
mod keypair;
mod program;

use errors::Error;
use files::{default_keypair_path, pick_file, FileType};
use keypair::load_keypair_from_file;
use solana_sdk::signature::Keypair;

fn main() -> iced::Result {
    BlichDeployer::run(Settings::default())
}

const RPC_URL: &str = "https://api.devnet.solana.com";

struct BlichDeployer {
    pub keypair_path: Option<PathBuf>,
    pub keypair: Arc<Keypair>,
    pub program_path: Option<PathBuf>,
    pub rpc_client: Arc<RpcClient>,
    pub buffer_account: String,
    pub transactions: (usize, usize),
    pub is_deploying: bool,
    pub progress_receiver: Arc<Mutex<Option<mpsc::Receiver<(usize, usize)>>>>,
    pub error: Option<Error>,
}

#[derive(Debug, Clone)]
enum Message {
    PickProgramAuthority,
    LoadProgramAuthority(Result<PathBuf, Error>),
    PickProgram,
    LoadProgram(Result<PathBuf, Error>),
    DeployProgram(PathBuf),
    ProgramDeployed(Result<String, Error>),
    RpcClient(String),
    UpdateProgress(Progress),
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
                keypair: Keypair::new().into(),
                buffer_account: String::from("Non created buffer account"),
                transactions: (0, 0),
                progress_receiver: Arc::new(Mutex::new(None)),
                is_deploying: false,
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
            Message::DeployProgram(program_path) => {
                self.transactions = (0,0);
                let (progress_sender, progress_receiver) = mpsc::channel::<(usize, usize)>(256);
                let is_deploying = true;
                let values = Arc::new(BlichDeployer {
                    keypair_path: self.keypair_path.clone(),
                    keypair: self.keypair.clone(),
                    program_path: self.program_path.clone(),
                    rpc_client: self.rpc_client.clone(),
                    buffer_account: self.buffer_account.clone(),
                    transactions: self.transactions,
                    error: self.error.clone(),
                    is_deploying,
                    progress_receiver: Arc::new(Mutex::new(None)),
                });

                self.progress_receiver = Arc::new(Mutex::new(Some(progress_receiver)));
                self.is_deploying = is_deploying;
                Command::perform(
                    process_transactions(program_path, Arc::clone(&values), progress_sender),
                    Message::ProgramDeployed,
                )
            }
            Message::ProgramDeployed(Ok(buffer)) => {
                println!("Deployed!");
                self.buffer_account = buffer;
                Command::none()
            }
            Message::ProgramDeployed(Err(e)) => {
                self.error = Some(e);
                Command::none()
            }
            Message::UpdateProgress(progress) => {
                match progress {
                    Progress::Sending { sent, total } => {
                        self.transactions = (sent, total);
                    }
                    Progress::Completed => {
                        self.is_deploying = false;
                        self.progress_receiver = Arc::new(Mutex::new(None));
                    }
                    Progress::Idle=>{
                        println!("Starting")
                    }
                }
                Command::none()
            }
            Message::RpcClient(rpc_client) => {
                self.rpc_client = Arc::new(RpcClient::new(rpc_client));
                Command::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        match self.is_deploying {
            true => {
                let progress_receiver = Arc::clone(&self.progress_receiver);
                progress_subscription(progress_receiver).map(Message::UpdateProgress)
            }
            false => Subscription::none(),
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
        let buffer_acc = buffer_address(&self.buffer_account);
        let display_error = error(&self.error);
        let load_program_btn = load_program_btn();
        let tx_progress = tx_progress(self.transactions.0, self.transactions.1);
        let set_rpc_client = handle_rpc_url(&self.rpc_client.url());
        let deploy_program_btn = deploy_program_btn(program_path);

        container(
            column![
                display_pubkey,
                buffer_acc,
                set_rpc_client,
                load_keypair_btn,
                load_program_btn,
                display_error,
                deploy_program_btn,
                tx_progress,
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
