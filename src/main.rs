use components::{buffer_address, deploy_program_btn, error, tx_progress};
use iced::{
    executor,
    widget::{column, container},
    Application, Command, Element, Renderer, Settings, Subscription, Theme,
};
use programs::{progress_subscription, BPrograms, Progress};
use settings::BSettings;
use solana_client::nonblocking::rpc_client::RpcClient;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};
mod components;
mod errors;
mod files;
mod keypair;
mod programs;
mod settings;

use errors::Error;
use files::{default_keypair_path, pick_file, FileType};
use keypair::load_keypair_from_file;
use solana_sdk::{signature::Keypair, signer::Signer};

fn main() -> iced::Result {
    Blich::run(Settings::default())
}

struct Blich {
    pub settings: BSettings,
    pub programs: BPrograms,
    pub error: Option<Error>,
}

impl Default for Blich {
    fn default() -> Self {
        Self {
            settings: BSettings::default(),
            programs: BPrograms::default(),
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    PickProgramAuthority,
    LoadProgramAuthority(Result<PathBuf, Error>),
    PickProgram,
    LoadProgram(Result<PathBuf, Error>),
    DeployProgram,
    ProgramDeployed(Result<Arc<Keypair>, Error>),
    RpcClient(String),
    UpdateProgress(Progress),
}

impl Application for Blich {
    type Executor = executor::Default;

    type Message = Message;

    type Theme = Theme;

    type Flags = ();

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        (
            Blich::default(),
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
                self.settings.keypair_path = Some(path.to_path_buf());
                self.settings.keypair = load_keypair_from_file(path.to_path_buf()).into();
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
                self.settings.program_path = Some(path.to_path_buf());
                Command::none()
            }
            Message::LoadProgram(Err(err)) => {
                self.error = Some(err);
                Command::none()
            }
            Message::DeployProgram => {
                self.programs.transactions = (0, 0);
                let (progress_sender, progress_receiver) = mpsc::channel::<(usize, usize)>(10000);
                self.programs.receiver_data_channel = Arc::new(Mutex::new(Some(progress_receiver)));
                self.programs.is_deploying = true;
                Command::perform(
                    BPrograms::create_buffer_and_write_data(
                        self.programs.clone(),
                        self.settings.clone(),
                        progress_sender,
                    ),
                    Message::ProgramDeployed,
                )
            }
            Message::ProgramDeployed(Ok(buffer)) => {
                println!("Deployed!");
                self.programs.buffer_account = buffer;
                self.programs.is_deployed = true;
                self.programs.is_deploying = false;
                Command::none()
            }
            Message::ProgramDeployed(Err(e)) => {
                self.error = Some(e);
                Command::none()
            }
            Message::UpdateProgress(progress) => {
                match progress {
                    Progress::Sending { sent, total } => {
                        self.programs.is_deployed = false;
                        self.programs.transactions = (sent, total);
                    }
                    Progress::Completed => {
                        self.programs.is_deployed = true;
                        self.programs.receiver_data_channel = Arc::new(Mutex::new(None));
                    }
                    Progress::Idle => {
                        println!("Starting")
                    }
                }
                Command::none()
            }
            Message::RpcClient(rpc_client) => {
                self.settings.rpc_client = Arc::new(RpcClient::new(rpc_client));
                Command::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        match self.programs.is_deploying {
            true => {
                let progress_receiver = Arc::clone(&self.programs.receiver_data_channel);
                progress_subscription(progress_receiver).map(Message::UpdateProgress)
            }
            false => Subscription::none(),
        }
    }

    fn view(&self) -> Element<'_, Self::Message, Renderer<Self::Theme>> {
        let settings = self.settings.view();
        let is_deployed = self.programs.view();
        let buffer_acc = buffer_address(&self.programs.buffer_account.clone().pubkey().to_string());
        let display_error = error(&self.error);
        let tx_progress = tx_progress(self.programs.transactions.0, self.programs.transactions.1);
        let deploy_program_btn = deploy_program_btn();

        container(
            column![
                settings,
                buffer_acc,
                deploy_program_btn,
                tx_progress,
                display_error,
                is_deployed
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
