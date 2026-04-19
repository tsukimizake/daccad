use iced::widget::button::{self, Status};
use iced::{Background, Border, Color, Shadow};

pub fn dark_button_style(_theme: &iced::Theme, status: Status) -> button::Style {
    let bg = Color::from_rgb(0.24, 0.24, 0.26);
    let bg_hover = Color::from_rgb(0.34, 0.34, 0.36);
    let bg_pressed = Color::from_rgb(0.18, 0.18, 0.20);
    let text = Color::from_rgb(0.88, 0.88, 0.90);

    match status {
        Status::Active => button::Style {
            background: Some(Background::Color(bg)),
            text_color: text,
            border: Border::default().rounded(4),
            shadow: Shadow::default(),
        },
        Status::Hovered => button::Style {
            background: Some(Background::Color(bg_hover)),
            text_color: text,
            border: Border::default().rounded(4),
            shadow: Shadow::default(),
        },
        Status::Pressed => button::Style {
            background: Some(Background::Color(bg_pressed)),
            text_color: text,
            border: Border::default().rounded(4),
            shadow: Shadow::default(),
        },
        Status::Disabled => button::Style {
            background: Some(Background::Color(Color::from_rgb(0.18, 0.18, 0.20))),
            text_color: Color::from_rgb(0.45, 0.45, 0.47),
            border: Border::default().rounded(4),
            shadow: Shadow::default(),
        },
    }
}

pub fn dark_button<'a, Msg: Clone + 'a>(label: &'a str) -> iced::widget::Button<'a, Msg> {
    iced::widget::button(label).style(dark_button_style)
}
