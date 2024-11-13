use std::path::PathBuf;

use crate::{errors::Error, Message};
use iced::{
    color,
    widget::{button, column, progress_bar, row, text},
    Element,
};

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

pub fn deploy_program_btn() -> Element<'static, Message> {
    let load_btn = button("Deploy").on_press(Message::DeployProgram);
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
