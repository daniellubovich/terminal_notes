use crate::navigation_state::SortField;

pub enum Field {
    Size,
    Name,
    Modified,
}

pub struct Column {
    pub field: Field,
    pub name: String,
    pub sort_field: SortField,
}

impl Column {
    pub fn get_field(&self) -> &Field {
        &self.field
    }

    pub fn get_name(&self) -> &String {
        &self.name
    }

    pub fn get_sort_field(&self) -> &SortField {
        &self.sort_field
    }
}

pub trait Columnar {
    fn get_value(&self, column: &Column) -> String;
}

pub mod table {
    use crate::NavigationState;
    use crate::{Column, Columnar, SortDir};
    use log::debug;
    use std::rc::Rc;
    use termion::{color, cursor};

    pub fn get_column_width(rows: &Vec<Rc<dyn Columnar>>, column: &Column) -> usize {
        let mut width = column.get_name().len() + 4;
        for row in rows {
            let col_w = row.get_value(column).len() + 4;
            if col_w > width {
                width = col_w;
            }
        }
        width
    }

    pub fn draw_header(
        rows: &Vec<Rc<dyn Columnar>>,
        columns: &Vec<Column>,
        state: &NavigationState,
    ) -> String {
        let sort_indicator = match state.get_sort_dir() {
            SortDir::Desc => "↓",
            SortDir::Asc => "↑",
        };

        let mut header_str = format!(
            "{clear}{goto}{color}",
            goto = cursor::Goto(1, 1),
            clear = termion::clear::All,
            color = color::Fg(color::Yellow),
        );

        for column in columns {
            if *column.get_sort_field() == state.sort_field {
                header_str = format!(
                    "{header_str}{value:<width$}",
                    value = format!("{} {}", column.get_name(), sort_indicator),
                    width = get_column_width(rows, column),
                );
            } else {
                header_str = format!(
                    "{header_str}{value:<width$}",
                    value = column.get_name(),
                    width = get_column_width(rows, column),
                );
            }
        }

        format!("{header_str}{reset}\n", reset = color::Fg(color::Reset))
    }

    pub fn draw_footer(footer: &str, state: &NavigationState) -> String {
        // Print the command prompt at the bottom of the terminal.
        let window_size = state.get_window_size();
        let footer_render_index = window_size + 3;
        debug!("Rendering footer at position: {}", footer_render_index);
        format!(
            "{goto}{footer}",
            goto = cursor::Goto(1, footer_render_index),
            footer = footer
        )
    }

    pub fn draw(
        rows: &Vec<Rc<dyn Columnar>>,
        columns: &Vec<Column>,
        footer: &str,
        state: &NavigationState,
    ) -> String {
        let (h1, h2) = state.get_visible_window();

        let iter = IntoIterator::into_iter(rows);
        let mut render_index: u16 = 2;
        let mut table_str = String::new();
        for (index, row) in iter.enumerate() {
            if index < h1.into() || index > h2.into() {
                continue;
            }

            let mut row_str = String::new();

            if state.get_selected_index() == index {
                row_str = format!(
                    "{highlight}{fontcolor}",
                    highlight = color::Bg(color::White),
                    fontcolor = color::Fg(color::Black),
                );
            }

            row_str = format!("\r{row_str}");

            for column in columns {
                row_str = format!(
                    "{row_str}{value:<width$}",
                    value = row.get_value(column),
                    width = get_column_width(rows, column)
                );
            }

            table_str = format!(
                "{table_str}{row_str}\r\n{reset_highlight}{reset_fontcolor}",
                reset_highlight = color::Bg(color::Reset),
                reset_fontcolor = color::Fg(color::Reset)
            );

            render_index = render_index.saturating_add(1);
        }

        format!(
            "{header_str}{table_str}{footer}",
            header_str = draw_header(rows, columns, state),
            table_str = table_str,
            footer = draw_footer(footer, state),
        )
    }
}
