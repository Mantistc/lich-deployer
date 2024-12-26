use components::error;
use iced::{
    clipboard,
    widget::{column, container},
    Element, Subscription, Task, Theme,
};
use programs::{get_program_bytes, BPrograms, Progress};
use settings::{keypair_balance, BSettings};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signature::Signature;
use std::sync::Arc;
use std::{path::PathBuf, time::Duration};
use tokio::time;
mod components;
mod errors;
mod files;
mod instructions;
mod keypair;
mod programs;
mod settings;
mod transactions;

use errors::Error;
use files::{default_keypair_path, pick_file, FileType};
use keypair::load_keypair_from_file;

fn main() -> iced::Result {
    iced::application(Blich::title, Blich::update, Blich::view)
        .theme(Blich::theme)
        .subscription(Blich::subscription)
        .run_with(Blich::new)
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
    PickProgramAccount,
    LoadProgramAccount(Result<PathBuf, Error>),
    LoadProgramAuthority(Result<PathBuf, Error>),
    AuthoritySolBalance(Result<u64, Error>),
    PickProgram,
    LoadProgram(Result<PathBuf, Error>),
    WriteData,
    RpcClient(String),
    UpdateProgress(Result<Progress, Error>),
    CopyToCliboard(String),
    ErrorCleared,
    DeployProgram,
    DeployedProgramSignature(Result<Signature, Error>),
}

impl Blich {
    fn new() -> (Self, Task<Message>) {
        (
            Blich::default(),
            Task::perform(
                async { Ok(default_keypair_path()) },
                Message::LoadProgramAuthority,
            ),
        )
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::PickProgramAuthority => {
                Task::perform(pick_file(FileType::Keypair), Message::LoadProgramAuthority)
            }
            Message::LoadProgramAuthority(Ok(path)) => {
                self.settings.keypair_path = Some(path.to_path_buf());
                self.settings.keypair = load_keypair_from_file(path.to_path_buf()).into();
                Task::perform(
                    keypair_balance(path, self.settings.rpc_client.clone()),
                    Message::AuthoritySolBalance,
                )
            }
            Message::LoadProgramAuthority(Err(err)) => {
                self.error = Some(err);
                Task::perform(Blich::clear_error(), |_| Message::ErrorCleared)
            }
            Message::PickProgramAccount => {
                Task::perform(pick_file(FileType::Keypair), Message::LoadProgramAccount)
            }
            Message::LoadProgramAccount(Ok(path_buf)) => {
                self.programs.program_account = Some(load_keypair_from_file(path_buf).into());
                Task::none()
            }
            Message::LoadProgramAccount(Err(err)) => {
                self.error = Some(err);
                Task::perform(Blich::clear_error(), |_| Message::ErrorCleared)
            }
            Message::AuthoritySolBalance(Ok(balance)) => {
                self.settings.balance = Some(balance);
                Task::none()
            }
            Message::AuthoritySolBalance(Err(e)) => {
                self.error = Some(e);
                Task::perform(Blich::clear_error(), |_| Message::ErrorCleared)
            }
            Message::PickProgram => {
                Task::perform(pick_file(FileType::Program), Message::LoadProgram)
            }
            Message::LoadProgram(Ok(path)) => {
                self.settings.program_path = Some(path.to_path_buf());
                let program_path = self.settings.program_path.as_deref();
                if let Some(path) = program_path {
                    self.programs.program_bytes =
                        get_program_bytes(path.to_str().expect("A valid path is expected"))
                            .unwrap_or(Vec::new())
                }
                Task::none()
            }
            Message::LoadProgram(Err(err)) => {
                self.error = Some(err);
                Task::perform(Blich::clear_error(), |_| Message::ErrorCleared)
            }
            Message::WriteData => {
                self.programs.signature = None;
                self.programs.is_writing_data = true;
                self.programs.is_data_writed = false;
                self.programs.transactions = (0, 0);
                Task::none()
            }
            Message::UpdateProgress(progress) => {
                match progress {
                    Ok(Progress::Sending { sent, total }) => {
                        self.programs.transactions = (sent, total);
                    }
                    Ok(Progress::Completed { buffer_account }) => {
                        println!("Data Writed!");
                        self.programs.transactions = (0, 0);
                        self.programs.buffer_account = buffer_account;
                        self.programs.is_data_writed = true;
                        self.programs.is_writing_data = false;
                        return Task::perform(
                            keypair_balance(
                                self.settings
                                    .keypair_path
                                    .clone()
                                    .unwrap_or(default_keypair_path()),
                                self.settings.rpc_client.clone(),
                            ),
                            Message::AuthoritySolBalance,
                        );
                    }
                    Ok(Progress::Idle) => {
                        println!("Starting")
                    }
                    Err(e) => {
                        self.error = Some(e);
                        self.programs.transactions = (0, 0);
                        self.programs.is_data_writed = false;
                        self.programs.is_writing_data = false;
                        return Task::perform(Blich::clear_error(), |_| Message::ErrorCleared);
                    }
                }
                Task::none()
            }
            Message::DeployProgram => Task::perform(
                BPrograms::deploy_or_upgrade(self.programs.clone(), self.settings.clone()),
                Message::DeployedProgramSignature,
            ),
            Message::DeployedProgramSignature(Ok(signature)) => {
                self.programs.signature = Some(signature);
                Task::perform(
                    keypair_balance(
                        self.settings
                            .keypair_path
                            .clone()
                            .unwrap_or(default_keypair_path()),
                        self.settings.rpc_client.clone(),
                    ),
                    Message::AuthoritySolBalance,
                )
            }
            Message::DeployedProgramSignature(Err(err)) => {
                self.error = Some(err);
                Task::perform(Blich::clear_error(), |_| Message::ErrorCleared)
            }
            Message::RpcClient(rpc_client) => {
                self.settings.rpc_client = Arc::new(RpcClient::new(rpc_client));
                Task::none()
            }
            Message::CopyToCliboard(value_to_copy) => clipboard::write(value_to_copy.to_string()),
            Message::ErrorCleared => {
                self.error = None;
                Task::none()
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        match self.programs.is_writing_data {
            true => Progress::run_susbcription(1, self.programs.clone(), self.settings.clone())
                .map(|values| Message::UpdateProgress(values.1)),
            false => Subscription::none(),
        }
    }

    fn view(&self) -> Element<Message> {
        let settings = self.settings.view(&self.programs);
        let is_data_writed = self.programs.deployed_message_element();
        let deploy_btn = self.programs.deploy_or_upgrade_btn();
        let buffer_acc = self.programs.buffer_address();
        let display_error = error(&self.error);
        let tx_progress = self.programs.tx_progress();
        let write_data_btn = self.programs.write_data_btn();
        let signature = self.programs.signature_text_with_copy();

        container(
            column![
                settings,
                buffer_acc,
                tx_progress,
                write_data_btn,
                display_error,
                is_data_writed,
                deploy_btn,
                signature
            ]
            .spacing(5),
        )
        .padding(30)
        .into()
    }

    fn title(&self) -> String {
        String::from("Program Deployer")
    }

    fn theme(&self) -> Theme {
        Theme::Dracula
    }

    pub async fn clear_error() -> () {
        time::sleep(Duration::from_secs(5)).await;
    }
}
