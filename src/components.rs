use crate::{errors::Error, Message};
use iced::{
    color,
    widget::{button, column, progress_bar, row, text},
    Element,
};


pub fn tx_progress(current: usize, total: usize) -> Element<'static, Message> {
    let label = text(format!("Transaction progress: ",))
        .size(14)
        .color(color!(0x30cbf2));
    let values = text(format!("{}/{}", current, total)).size(14);
    let progress_bar = progress_bar(0.0..=total as f32, current as f32);
    let counter = row![label, values];
    let container = column![counter, progress_bar];
    container.into()
}

pub fn copy_to_cliboard_btn(text: &str) -> Element<'static, Message> {
    let copy_btn = button("Copy").on_press(Message::CopyToCliboard(text.to_string()));
    copy_btn.into()
}

pub fn error(error: &Option<Error>) -> Element<'static, Message> {
    let error_message = if let Some(ref error) = error {
        text(format!("Error: {:?}", error))
            .size(14)
            .color(color!(0xf75757))
    } else {
        text("").size(1)
    };
    error_message.into()
}
