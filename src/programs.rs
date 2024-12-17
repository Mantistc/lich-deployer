use iced::futures::channel::mpsc::Sender;
use iced::futures::future::join_all;
use iced::futures::{Stream, StreamExt};
use iced::stream::try_channel;
use iced::widget::{button, column, progress_bar, row, text};
use iced::{color, Alignment, Element, Subscription};
use solana_client::{rpc_client::SerializableTransaction, rpc_config::RpcSendTransactionConfig};
use solana_sdk::transaction::Transaction;
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    signature::{Keypair, Signature},
    signer::Signer,
};
use solana_transaction_status::UiTransactionEncoding;
use std::hash::Hash;
use std::{fs, sync::Arc, time::Duration};
use tokio::task::JoinHandle;
use tokio::{spawn, time};

use crate::components::copy_to_cliboard_btn;
use crate::instructions::{create_buffer_account, deploy_program, upgrade_program, write_data};
use crate::settings::BSettings;
use crate::transactions::send_tx_and_verify_status;
use crate::{errors::Error, Message};

pub const BYTES_PER_CHUNK: usize = 1012;
pub const PROGRAM_EXTRA_SPACE: usize = 45;

#[derive(Debug, Clone)]
pub struct BPrograms {
    pub buffer_account: Arc<Keypair>,
    pub program_account: Option<Arc<Keypair>>,
    pub program_bytes: Vec<u8>,
    pub transactions: (usize, usize),
    pub is_data_writed: bool,
    pub is_writing_data: bool,
    pub signature: Option<Signature>,
}

impl Default for BPrograms {
    fn default() -> Self {
        Self {
            buffer_account: Keypair::new().into(),
            program_account: None,
            program_bytes: Vec::new(),
            transactions: (0, 0),
            is_data_writed: false,
            is_writing_data: false,
            signature: None,
        }
    }
}

pub const SEND_CFG: RpcSendTransactionConfig = RpcSendTransactionConfig {
    skip_preflight: true,
    preflight_commitment: Some(CommitmentLevel::Finalized),
    encoding: Some(UiTransactionEncoding::Base64),
    max_retries: Some(3),
    min_context_slot: None,
};

impl BPrograms {
    pub async fn create_buffer_and_write_data(
        self,
        settings: BSettings,
        mut output: Sender<Progress>,
    ) -> Result<(), Error> {
        let _ = output.try_send(Progress::Idle);
        let buffer_acc = self.buffer_account;

        let authority = &settings.keypair;
        let rpc_client = &settings.rpc_client;

        let (recent_blockhash, _) = rpc_client
            .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
            .await
            .map_err(|e| Error::RpcError(e))?;

        let lamports = rpc_client
            .get_minimum_balance_for_rent_exemption(self.program_bytes.len() + PROGRAM_EXTRA_SPACE)
            .await
            .unwrap_or(0);

        if self.program_bytes.len() == 0 {
            println!("error");
            return Err(Error::InvalidProgramLen);
        }

        let buffer_acc_init_tx = create_buffer_account(
            &buffer_acc,
            &authority,
            lamports,
            &self.program_bytes,
            recent_blockhash,
        )
        .map_err(|e| Error::InstructionError(e))?;

        let _signature =
            send_tx_and_verify_status(&rpc_client, &buffer_acc_init_tx, SEND_CFG).await;

        let (updated_blockhash, _) = rpc_client
            .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
            .await
            .map_err(|e| Error::RpcError(e))?;

        let mut write_data_txs = write_data(
            &buffer_acc.pubkey(),
            &self.program_bytes,
            &authority,
            updated_blockhash,
        );

        let sleep_between_send = 25; // 15 ms to await between each send
        let batch_size = 250;

        let mut tx_sent = 0;
        loop {
            for transaction in &write_data_txs {
                tx_sent += 1;

                let _ = output.try_send(Progress::Sending {
                    sent: tx_sent,
                    total: write_data_txs.len(),
                });
                let client = rpc_client.clone();
                let tx = transaction.clone();
                spawn(async move {
                    let _ = client.send_transaction_with_config(&tx, SEND_CFG).await;
                });
                time::sleep(Duration::from_millis(sleep_between_send)).await;
            }

            time::sleep(Duration::from_secs(5)).await;

            let tx_signatures: Vec<Signature> = write_data_txs
                .iter()
                .map(|tx| *tx.get_signature())
                .collect();
            let mut tx_signatures_batches = get_vec_with_batched_data(batch_size, &tx_signatures);

            let check_failed_tx_tasks: Vec<JoinHandle<Vec<Signature>>> = tx_signatures_batches
                .iter_mut()
                .map(|chunk_signature| {
                    let rpc_client = rpc_client.clone();
                    let mut chunk_signatures = chunk_signature.clone();

                    spawn(async move {
                        let mut retrys = 0;
                        let mut tx_to_retry = Vec::new();
                        let max_retrys = 10;

                        while retrys < max_retrys {
                            let status_vec = rpc_client
                                .get_signature_statuses(&chunk_signatures)
                                .await
                                .ok()
                                .map(|v| v.value)
                                .unwrap_or_default();
                            let mut failed_signatures = Vec::new();

                            for (i, status) in status_vec.iter().enumerate() {
                                if status.as_ref().map_or(true, |c| {
                                    c.err.is_some()
                                        || (c.confirmation_status.is_none()
                                            && retrys == max_retrys - 1)
                                }) {
                                    failed_signatures.push(chunk_signatures[i]);
                                }
                            }
                            chunk_signatures.retain(|signature| {
                                let keep = !failed_signatures.contains(signature);
                                if !keep {
                                    tx_to_retry.push(*signature);
                                }
                                keep
                            });

                            if chunk_signatures.is_empty() {
                                break;
                            }

                            time::sleep(Duration::from_millis(200)).await;
                            retrys += 1;
                        }
                        tx_to_retry
                    })
                })
                .collect();

            let results: Vec<Vec<Signature>> = join_all(check_failed_tx_tasks)
                .await
                .into_iter()
                .filter_map(Result::ok)
                .collect();

            let tx_to_retry: Vec<Signature> = results.into_iter().flatten().collect();

            write_data_txs = write_data_txs
                .drain(..)
                .filter(|tx| tx_to_retry.contains(tx.get_signature()))
                .collect();

            let (updated_blockhash, _) = rpc_client
                .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
                .await
                .map_err(|e| Error::RpcError(e))?;

            for transaction in write_data_txs.iter_mut() {
                transaction.sign(&[&authority], updated_blockhash);
            }

            if tx_to_retry.is_empty() {
                let _ = output.try_send(Progress::Completed {
                    buffer_account: buffer_acc,
                });
                break;
            }

            tx_sent = 0;
        }
        Ok(())
    }

    pub async fn deploy_or_upgrade(self, settings: BSettings) -> Result<Signature, Error> {
        // first check if the program account is initialized
        let rpc_client = &settings.rpc_client;
        let program_account = if let Some(valid_program_acc) = &self.program_account {
            valid_program_acc
        } else {
            return Err(Error::ProgramAccountNotLoaded);
        };

        // if its err means that the account is not initialized yet and there is no data related to
        let has_data = !rpc_client
            .get_account(&program_account.pubkey())
            .await
            .is_err();

        let (blockhash, _) = rpc_client
            .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
            .await
            .map_err(|e| Error::RpcError(e))?;

        let tx: Transaction;

        // so, if has data, we just upgrade the program
        if has_data {
            tx = upgrade_program(
                &program_account,
                &self.buffer_account.pubkey(),
                &settings.keypair,
                blockhash,
            );
        } else {
            // if not, we deploy, in this part the program keypair needs to sign
            let lamports = rpc_client
                .get_minimum_balance_for_rent_exemption(
                    self.program_bytes.len() + PROGRAM_EXTRA_SPACE,
                )
                .await
                .unwrap_or(0);

            tx = deploy_program(
                &settings.keypair,
                &program_account,
                &self.buffer_account.pubkey(),
                &self.program_bytes,
                lamports,
                blockhash,
            )?;
        }
        let signature = send_tx_and_verify_status(&rpc_client, &tx, SEND_CFG).await?;
        println!("signature: {}", signature.to_string());
        Ok(signature)
    }

    // ------> UI COMPONENTS <------ //

    pub fn deployed_message_element(&self) -> Element<Message> {
        let is_data_writed = if self.is_data_writed {
            text("Data writed successfully").size(14)
        } else {
            text("").size(14)
        };
        is_data_writed.into()
    }

    pub fn buffer_address(&self) -> Element<Message> {
        let buffer_str = self.buffer_account.pubkey().to_string();
        let label = text(format!("Buffer Address: ",))
            .size(14)
            .color(color!(0x30cbf2));
        let value = text(buffer_str.clone()).size(14);
        let copy_btn = copy_to_cliboard_btn(&buffer_str);
        let value_with_copy_btn_row = row![value, copy_btn]
            .spacing(10)
            .align_y(iced::Alignment::Center);

        let container = column![label, value_with_copy_btn_row];
        container.into()
    }

    pub fn write_data_btn(&self) -> Element<Message> {
        let write_data_btn = button("Write data").on_press(Message::WriteData);
        write_data_btn.into()
    }

    pub fn tx_progress(&self) -> Element<'static, Message> {
        let current_tx_sent = self.transactions.0;
        let total_to_send = self.transactions.1;
        let label = text(format!("Transaction progress: ",))
            .size(14)
            .color(color!(0x30cbf2));
        let values = text(format!("{}/{}", current_tx_sent, total_to_send)).size(14);
        let progress_bar = progress_bar(0.0..=total_to_send as f32, current_tx_sent as f32);
        let counter = row![label, values];
        let container = column![counter, progress_bar];
        container.into()
    }

    pub fn deploy_or_upgrade_btn(&self) -> Element<Message> {
        if self.is_data_writed {
            let deploy = button("Deploy").on_press(Message::DeployProgram);
            deploy.into()
        } else {
            let deploy =
                text("To be able to deploy, the buffer account needs the data writed").size(14);
            deploy.into()
        }
    }

    pub fn signature_text_with_copy(&self) -> Element<Message> {
        if let Some(signature) = self.signature {
            let signature_text = text(format!("tx: {}", signature.to_string()));
            let copy_btn = copy_to_cliboard_btn(&signature.to_string());
            let row = row![signature_text, copy_btn]
                .spacing(5)
                .align_y(Alignment::Center);
            row.into()
        } else {
            text("").into()
        }
    }
}

pub fn get_program_bytes(program_path: &str) -> Result<Vec<u8>, Error> {
    match fs::read(program_path) {
        Ok(bytes) => {
            println!("bytes len: {}", bytes.len());
            if bytes.len() == 0 {
                return Err(Error::InvalidProgramLen);
            }
            Ok(bytes)
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            return Err(Error::UnexpectedError);
        }
    }
}

pub fn get_vec_with_batched_data<T: Clone>(batch_size: usize, base_vec: &Vec<T>) -> Vec<Vec<T>> {
    let mut offset = 0;
    let mut vec_with_batchs = Vec::new();

    while offset < base_vec.len() {
        let chunk_end = (offset + batch_size).min(base_vec.len());

        let chunk = base_vec[offset..chunk_end].to_vec();

        offset += batch_size;
        vec_with_batchs.push(chunk);
    }
    vec_with_batchs
}

#[derive(Debug, Clone, PartialEq)]
pub enum Progress {
    Idle,
    Sending { sent: usize, total: usize },
    Completed { buffer_account: Arc<Keypair> },
}

impl Progress {
    pub fn sending_tx_progress_sub(
        programs: BPrograms,
        settings: BSettings,
    ) -> impl Stream<Item = Result<Progress, Error>> {
        try_channel(1500, move |output| async move {
            let result = BPrograms::create_buffer_and_write_data(programs, settings, output).await;
            if let Err(_e) = result {
                return Err(Error::UnexpectedError);
            } else {
                Ok(())
            }
        })
    }

    pub fn run_susbcription<I: 'static + Hash + Copy + Send + Sync>(
        id: I,
        programs: BPrograms,
        settings: BSettings,
    ) -> iced::Subscription<(I, Result<Progress, Error>)> {
        Subscription::run_with_id(
            id,
            Progress::sending_tx_progress_sub(programs, settings)
                .map(move |progress| (id, progress)),
        )
    }
}
