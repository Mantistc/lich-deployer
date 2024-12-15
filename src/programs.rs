use iced::futures::channel::mpsc::Sender;
use iced::futures::{Stream, StreamExt};
use iced::stream::try_channel;
use iced::widget::{button, column, row, text};
use iced::{color, Element, Subscription};
use solana_client::{rpc_client::SerializableTransaction, rpc_config::RpcSendTransactionConfig};
use solana_sdk::{
    commitment_config::{CommitmentConfig, CommitmentLevel},
    signature::{Keypair, Signature},
    signer::Signer,
};
use solana_transaction_status::UiTransactionEncoding;
use std::hash::Hash;
use std::{fs, sync::Arc, time::Duration};
use tokio::{spawn, time};

use crate::components::copy_to_cliboard_btn;
use crate::instructions::{create_buffer_account, write_data};
use crate::settings::BSettings;
use crate::transactions::send_tx_and_verify_status;
use crate::{errors::Error, Message};

pub const BYTES_PER_CHUNK: usize = 1012;
pub const PROGRAM_EXTRA_SPACE: usize = 45;

#[derive(Debug, Clone)]
pub struct BPrograms {
    pub buffer_account: Arc<Keypair>,
    pub program_bytes: Vec<u8>,
    pub transactions: (usize, usize),
    pub is_deployed: bool,
    pub is_deploying: bool,
}

impl Default for BPrograms {
    fn default() -> Self {
        Self {
            buffer_account: Keypair::new().into(),
            program_bytes: Vec::new(),
            transactions: (0, 0),
            is_deployed: false,
            is_deploying: false,
        }
    }
}

impl BPrograms {
    pub async fn create_buffer_and_write_data(
        self,
        settings: BSettings,
        mut output: Sender<Progress>,
    ) -> Result<(), Error> {
        let _ = output.try_send(Progress::Idle);
        let buffer_acc = self.buffer_account;

        let authority = settings.keypair.clone();
        let rpc_client = settings.rpc_client.clone();

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

        let send_cfg = RpcSendTransactionConfig {
            skip_preflight: false,
            preflight_commitment: Some(CommitmentLevel::Confirmed),
            encoding: Some(UiTransactionEncoding::Base64),
            max_retries: Some(3),
            min_context_slot: None,
        };

        let _signature =
            send_tx_and_verify_status(&rpc_client, &buffer_acc_init_tx, send_cfg).await;

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

        let mut tx_sent = 0;
        let sleep_between_send = 15; // 5 ms to await between each send

        // send all tx
        for transaction in &write_data_txs {
            tx_sent += 1;
            let _ = output.try_send(Progress::Sending {
                sent: tx_sent,
                total: write_data_txs.len(),
            });
            let client = rpc_client.clone();
            let config = send_cfg.clone();
            let tx = transaction.clone();
            spawn(async move {
                let _ = client.send_transaction_with_config(&tx, config).await;
            });
            time::sleep(Duration::from_millis(sleep_between_send)).await
        }

        // this is required because the method get_signature_statuses only accept a max of 256 signatures
        let batch_size = 256;
        // TODO: Get the new signed tx and update the tx_signatures_batches
        loop {
            let tx_signatures: Vec<Signature> = write_data_txs
                .clone()
                .into_iter()
                .map(|tx| *tx.get_signature())
                .collect();
            let tx_signatures_batches = get_vec_with_batched_data(batch_size, &tx_signatures);
            tx_sent = 0;
            let mut tx_to_retry = Vec::new();
            for chunk_signature in &tx_signatures_batches {
                let status_vec = &rpc_client
                    .get_signature_statuses(&chunk_signature)
                    .await
                    .map_err(|e| Error::RpcError(e))?
                    .value;

                for (i, status) in status_vec.iter().enumerate() {
                    if let Some(confirmation) = status {
                        if confirmation.err.is_some() {
                            tx_to_retry.push(tx_signatures[i]);
                        }
                    } else {
                        tx_to_retry.push(tx_signatures[i]);
                    }
                }
            }
            let save_last_value = write_data_txs;
            write_data_txs = Vec::new();

            for signature in &tx_to_retry {
                let mut tx_from_signature = None;

                for tx in save_last_value.clone() {
                    let write_tx_signature = tx.get_signature();
                    if *write_tx_signature == *signature {
                        println!("equal?: {:?}", *write_tx_signature == *signature);
                        tx_from_signature = Some(tx);
                        break;
                    }
                }
                if let Some(tx) = tx_from_signature {
                    write_data_txs.push(tx);
                }
            }

            let (updated_blockhash, _) = rpc_client
                .get_latest_blockhash_with_commitment(CommitmentConfig::confirmed())
                .await
                .map_err(|e| Error::RpcError(e))?;

            let tx_len = write_data_txs.len();

            for transaction in write_data_txs.iter_mut() {
                transaction.sign(&[&authority], updated_blockhash);
                tx_sent += 1;
                let _ = output.try_send(Progress::Sending {
                    sent: tx_sent,
                    total: tx_len,
                });
                let client = rpc_client.clone();
                let config = send_cfg.clone();
                let tx = transaction.clone();
                spawn(async move {
                    let _ = client.send_transaction_with_config(&tx, config).await;
                });
                time::sleep(Duration::from_millis(sleep_between_send)).await
            }
            println!(
                "tx sent: {}, total: {}, tx to retry: {}",
                tx_sent,
                write_data_txs.len(),
                tx_to_retry.len()
            );
            if tx_to_retry.len() == 0 {
                let _ = output.try_send(Progress::Completed {
                    buffer_account: buffer_acc,
                });
                break;
            }
            time::sleep(Duration::from_secs(5)).await
        }
        Ok(())
    }

    // ------> UI COMPONENTS <------ //

    pub fn deployed_message_element(&self) -> Element<Message> {
        let is_deployed = if self.is_deployed {
            text("Buffer account created & data writed")
                .size(14)
                .color(color!(0x30cbf2))
        } else {
            text("").size(14)
        };
        is_deployed.into()
    }

    pub fn program_size_element(&self) -> Element<Message> {
        let column = column![
            text("Program size: ").color(color!(0x30cbf2)).size(14),
            text(format!("{} bytes", self.program_bytes.len())).size(14)
        ];
        column.into()
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
        let write_data_btn = button("Write data").on_press(Message::DeployProgram);
        write_data_btn.into()
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
        try_channel(1000, move |output| async move {
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
