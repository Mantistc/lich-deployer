use crate::{errors::Error, Message};
use iced::{
    color,
    widget::{button, text},
    Element,
};

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
