#[derive(Eq, PartialEq, Clone)]
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
}

impl NavigationState {
    pub fn new(selected_index: usize) -> Self {
        NavigationState {
            selected_index,
            sort_field: SortField::Modified,
            sort_dir: SortDir::Asc,
        }
    }

    pub fn get_selected_index(&self) -> usize {
        self.selected_index
    }

    pub fn set_selected_index(&mut self, new_index: usize) {
        self.selected_index = new_index;
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
