use console::style;

/// Styling helpers for terminal output
pub fn bright_yellow(text: impl std::fmt::Display) -> console::StyledObject<String> {
    style(text.to_string()).bright().yellow()
}

pub fn bright_green(text: impl std::fmt::Display) -> console::StyledObject<String> {
    style(text.to_string()).bright().green()
}

pub fn bright_red(text: impl std::fmt::Display) -> console::StyledObject<String> {
    style(text.to_string()).bright().red()
}

pub fn cyan(text: impl std::fmt::Display) -> console::StyledObject<String> {
    style(text.to_string()).cyan()
}

pub fn dim(text: impl std::fmt::Display) -> console::StyledObject<String> {
    style(text.to_string()).dim()
}

pub fn bright(text: impl std::fmt::Display) -> console::StyledObject<String> {
    style(text.to_string()).bright()
}

pub fn magenta_bold(text: impl std::fmt::Display) -> console::StyledObject<String> {
    style(text.to_string()).magenta().bold()
}
