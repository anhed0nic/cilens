use comfy_table::modifiers::UTF8_ROUND_CORNERS;
use comfy_table::presets::UTF8_FULL;
use comfy_table::{Cell, Color as TableColor, ContentArrangement, Table};

/// Table and cell creation helpers
pub fn create_table() -> Table {
    let mut table = Table::new();
    table
        .load_preset(UTF8_FULL)
        .apply_modifier(UTF8_ROUND_CORNERS)
        .set_content_arrangement(ContentArrangement::Dynamic);
    table
}

pub fn color_coded_success_cell(rate: f64) -> Cell {
    let text = format!("{rate:.1}%");
    if rate > 80.0 {
        Cell::new(text).fg(TableColor::Green)
    } else if rate >= 50.0 {
        Cell::new(text).fg(TableColor::Yellow)
    } else {
        Cell::new(text).fg(TableColor::Red)
    }
}

pub fn color_coded_duration_cell(seconds: f64) -> Cell {
    let minutes = seconds / 60.0;
    let text = format!("{minutes:.1}min");
    if minutes <= 10.0 {
        Cell::new(text).fg(TableColor::Green)
    } else if minutes <= 15.0 {
        Cell::new(text).fg(TableColor::Yellow)
    } else {
        Cell::new(text).fg(TableColor::Red)
    }
}

pub fn color_coded_failure_cell(rate: f64) -> Cell {
    let text = format!("{rate:.1}%");
    if rate >= 50.0 {
        Cell::new(text).fg(TableColor::Red)
    } else if rate >= 25.0 {
        Cell::new(text).fg(TableColor::Yellow)
    } else {
        Cell::new(text).fg(TableColor::Green)
    }
}

pub fn color_coded_flakiness_cell(rate: f64) -> Cell {
    let text = format!("{rate:.1}%");
    if rate >= 10.0 {
        Cell::new(text).fg(TableColor::Red)
    } else if rate >= 5.0 {
        Cell::new(text).fg(TableColor::Yellow)
    } else {
        Cell::new(text).fg(TableColor::Green)
    }
}
