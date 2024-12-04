use log::debug;

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
    list_size: u16,
    pub sort_field: SortField,
    selected_index: usize,
    sort_dir: SortDir,
    visible_window: (u16, u16),
    window_buffer: u16,
}

#[allow(dead_code)]
impl NavigationState {
    pub fn new(selected_index: usize) -> Self {
        // TODO fix error handling and make the visible window size adjust each render,
        // so it handles terminal resizing.
        let (_, h) = termion::terminal_size().unwrap();

        let list_height = h - 2; // subtract 2 -- one for header, one for footer
        NavigationState {
            selected_index,
            sort_field: SortField::Modified,
            sort_dir: SortDir::Desc,
            visible_window: (0, list_height - 1), // subtract one since window is 0-based
            list_size: 0,
            window_buffer: 2,
        }
    }

    pub fn get_list_size(&self) -> u16 {
        self.list_size
    }

    pub fn get_visible_window(&self) -> (u16, u16) {
        self.visible_window
    }

    pub fn get_window_size(&self) -> u16 {
        self.visible_window.1 - self.visible_window.0
    }

    pub fn get_sort_dir(&self) -> &SortDir {
        &self.sort_dir
    }

    pub fn get_sort_field(&self) -> &SortField {
        &self.sort_field
    }

    pub fn get_window_buffer(&self) -> u16 {
        self.window_buffer
    }

    pub fn get_selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn increment_selected_index(&mut self, increment: usize) {
        let new_index = self.selected_index.saturating_add(increment);

        if (new_index as u16) < self.list_size {
            if self.visible_window.1 < ((new_index as u16) + self.window_buffer) {
                debug!(
                    "{} < {}",
                    self.visible_window.1,
                    ((new_index as u16) + self.window_buffer)
                );
                let mut window_start = self.visible_window.0;
                let visibility_range = self.visible_window.1 - self.visible_window.0;
                loop {
                    let window_end = window_start + visibility_range;
                    if window_end >= (new_index as u16) || window_end > self.list_size {
                        break;
                    }
                    window_start = window_start.saturating_add(1);
                }
                self.visible_window = (window_start, window_start + visibility_range);
            }

            debug!(
                "incrementing index - new_index:{},old index:{},vw:{},{}",
                new_index, self.selected_index, self.visible_window.0, self.visible_window.1
            );

            self.selected_index = new_index;
        }
    }

    pub fn decrement_selected_index(&mut self, decrement: usize) {
        let new_index = self.selected_index.saturating_sub(decrement);

        if self.visible_window.0 + self.window_buffer > (new_index as u16) {
            let mut window_start = self.visible_window.0;
            let visibility_range = self.visible_window.1 - self.visible_window.0;
            loop {
                if window_start + self.window_buffer <= (new_index as u16) || window_start == 0 {
                    break;
                }
                window_start = window_start.saturating_sub(1);
            }
            self.visible_window = (window_start, window_start + visibility_range);
        }

        debug!(
            "decrementing index - new_index:{},old index:{},vw:{},{}",
            new_index, self.selected_index, self.visible_window.0, self.visible_window.1
        );

        self.selected_index = new_index;
    }

    pub fn set_list_size(&mut self, list_size: u16) {
        self.list_size = list_size;
    }

    pub fn set_selected_index(&mut self, new_index: usize) {
        if new_index > self.selected_index {
            self.increment_selected_index(new_index - self.selected_index);
        } else {
            self.decrement_selected_index(self.selected_index - new_index);
        }
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
