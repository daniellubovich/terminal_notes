use std::rc::Rc;

use log::{debug, info};
use termion::{color, cursor};

use crate::navigation_state::{NavigationState, SortDir, SortField};

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

    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn get_sort_field(&self) -> &SortField {
        &self.sort_field
    }
}

pub trait Columnar {
    fn get_value(&self, column: &Column) -> String;
}

pub struct TableDisplay<'a> {
    pub rows: Vec<Rc<dyn Columnar>>,
    pub columns: Vec<Column>,
    pub state: &'a mut NavigationState,
    pub footer: String,
}

impl TableDisplay<'_> {
    pub fn new(
        rows: Vec<Rc<dyn Columnar>>,
        columns: Vec<Column>,
        state: &mut NavigationState,
        footer: String,
    ) -> TableDisplay {
        // This sucks. maybe there's a better way to merge navigation state with TableDisplay
        state.set_list_size(rows.len() as u16);

        TableDisplay {
            rows,
            columns,
            state,
            footer,
        }
    }

    pub fn set_rows(&mut self, rows: Vec<Rc<dyn Columnar>>) {
        self.rows = rows;
        self.state.set_list_size(self.rows.len() as u16);
    }

    pub fn get_column_width(&self, column: &Column) -> usize {
        let mut width = column.get_name().len() + 4;
        for row in &self.rows {
            let col_w = row.get_value(column).len() + 4;
            if col_w > width {
                width = col_w;
            }
        }
        width
    }

    pub fn draw_header(&self) -> String {
        let sort_indicator = match self.state.get_sort_dir() {
            SortDir::Desc => "↓",
            SortDir::Asc => "↑",
        };

        let mut header_str = format!(
            "{clear}{goto}{color}",
            goto = cursor::Goto(1, 1),
            clear = termion::clear::All,
            color = color::Fg(color::Yellow),
        );

        for column in &self.columns {
            if *column.get_sort_field() == self.state.sort_field {
                header_str = format!(
                    "{header_str}{value:<width$}",
                    value = format!("{} {}", column.get_name(), sort_indicator),
                    width = self.get_column_width(column),
                );
            } else {
                header_str = format!(
                    "{header_str}{value:<width$}",
                    value = column.get_name(),
                    width = self.get_column_width(column),
                );
            }
        }

        format!("{header_str}{reset}\n", reset = color::Fg(color::Reset))
    }

    pub fn draw_footer(&self) -> String {
        // Print the command prompt at the bottom of the terminal.
        let window_size = self.state.get_window_size();
        let footer_render_index = window_size + 1;
        info!("Rendering footer at position: {}", footer_render_index);
        format!(
            "{goto}{footer}",
            goto = cursor::Goto(1, footer_render_index),
            footer = self.footer
        )
    }

    pub fn draw(&self) -> String {
        let (h1, h2) = self.state.get_visible_window();

        let iter = IntoIterator::into_iter(&self.rows);
        let mut render_index: u16 = 2;
        let mut table_str = String::new();
        for (index, row) in iter.enumerate() {
            if index < h1.into() || index > (h2 - 2).into() {
                continue;
            }

            let mut row_str = String::new();

            if self.state.get_selected_index() == index {
                row_str = format!(
                    "{highlight}{fontcolor}",
                    highlight = color::Bg(color::White),
                    fontcolor = color::Fg(color::Black),
                );
            }

            row_str = format!("{goto}{row_str}", goto = cursor::Goto(1, render_index));

            for column in &self.columns {
                row_str = format!(
                    "{row_str}{value:<width$}",
                    value = row.get_value(column),
                    width = self.get_column_width(column)
                );
            }

            table_str = format!(
                "{table_str}{row_str}{reset_highlight}{reset_fontcolor}",
                reset_highlight = color::Bg(color::Reset),
                reset_fontcolor = color::Fg(color::Reset)
            );

            render_index = render_index.saturating_add(1);
        }


        format!(
            "{header_str}{table_str}{footer}", 
            header_str = self.draw_header(),
            table_str = table_str,
            footer = self.draw_footer(),
        )
    }
}
