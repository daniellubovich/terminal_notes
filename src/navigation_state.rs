use anyhow::bail;
use log::{debug, info};

#[derive(Eq, PartialEq)]
pub enum SortField {
    Modified,
    Size,
    Name,
}

#[derive(Eq, PartialEq)]
pub enum SortDir {
    Asc = 1,
    Desc = -1,
}

pub struct NavigationState {
    selected_index: usize,
    sort_dir: SortDir,
    pub sort_field: SortField,
    visible_window: (u16, u16),
    list_size: u16,
}

impl NavigationState {
    pub fn new(selected_index: usize) -> Self {
        let (_, h) = termion::terminal_size().unwrap(); // TODO Fix
        NavigationState {
            selected_index,
            sort_field: SortField::Modified,
            sort_dir: SortDir::Asc,
            visible_window: (0, h - 1),
            list_size: 0,
        }
    }

    pub fn set_list_size(&mut self, list_size: u16) {
        self.list_size = list_size;
    }

    pub fn get_selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn increment_selected_index(&mut self, increment: usize) {
        let new_index = self.selected_index.saturating_add(increment);

        let buffer = 3;

        if (new_index as u16) < self.list_size {
            info!(
                "incrementing:{},new_index:{},vw:{},{}",
                increment, new_index, self.visible_window.0, self.visible_window.1
            );
            if self.visible_window.1 < (new_index as u16) + buffer {
                let mut window_start = self.visible_window.0;
                let visibility_range = self.visible_window.1 - self.visible_window.0;
                loop {
                    debug!(
                        "incrementing vw:{},{},list_size:{}",
                        window_start,
                        window_start + visibility_range,
                        self.list_size
                    );
                    let window_end = window_start + visibility_range;
                    if window_end >= (new_index as u16) + buffer || window_end > self.list_size {
                        break;
                    }
                    window_start = window_start.saturating_add(1);
                }
                self.visible_window = (window_start, window_start + visibility_range);
            }

            self.selected_index = new_index;
        }
    }

    pub fn decrement_selected_index(&mut self, decrement: usize) {
        let new_index = self.selected_index.saturating_sub(decrement);
        self.selected_index = new_index;

        let buffer = 3;

        info!(
            "decrementing:{},new_index:{},vw:{},{}",
            decrement, new_index, self.visible_window.0, self.visible_window.1
        );

        if self.visible_window.0 + buffer > (new_index as u16) {
            let mut window_start = self.visible_window.0;
            let visibility_range = self.visible_window.1 - self.visible_window.0;
            loop {
                info!("decrementing vw:{},{}", new_index, window_start + buffer);
                if window_start + buffer <= (new_index as u16) || window_start == 0 {
                    break;
                }
                window_start = window_start.saturating_sub(1);
            }
            self.visible_window = (window_start, window_start + visibility_range);
        }
    }

    pub fn set_selected_index(&mut self, new_index: usize) {
        if new_index > self.selected_index {
            self.increment_selected_index(new_index - self.selected_index);
        } else {
            self.decrement_selected_index(self.selected_index - new_index);
        }
    }

    pub fn get_visible_window(&self) -> (u16, u16) {
        self.visible_window
    }

    pub fn get_sort_dir(&self) -> &SortDir {
        &self.sort_dir
    }

    pub fn get_sort_field(&self) -> &SortField {
        &self.sort_field
    }

    pub fn sort(&mut self, sort_field: SortField) {
        let mut sort_dir = SortDir::Desc;
        if self.sort_field == sort_field {
            if self.sort_dir == SortDir::Desc {
                sort_dir = SortDir::Asc;
            } else {
                sort_dir = SortDir::Desc;
            }
        }

        self.sort_dir = sort_dir;
        self.sort_field = sort_field;
    }
}
