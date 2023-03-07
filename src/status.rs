use tui::widgets::ListState;

///
#[derive(Copy, Clone, Hash, PartialEq, Eq, Debug)]
pub enum StatusItemType {
    ///
    New,
    ///
    Modified,
    ///
    Deleted,
    ///
    Normal,
}

///

#[derive(Clone)]
pub struct FolderStatefulList {
    pub entry: walkdir::DirEntry,
    pub state: StatusItemType,
}

///
pub struct StatefulList<T> {
    pub state: ListState,
    pub items: Vec<T>,
}

impl<T> StatefulList<T> {
    pub fn with_items(items: Vec<T>) -> StatefulList<T> {
        StatefulList {
            state: ListState::default(),
            items,
        }
    }

    pub fn next(&mut self, dist: usize) {
        let mut page_size = dist;
        if page_size > self.items.len() {
            page_size = self.items.len();
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i >= self.items.len() - page_size {
                    0
                } else {
                    i + page_size
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn previous(&mut self, dist: usize) {
        let mut page_size = dist;
        if page_size > self.items.len() {
            page_size = self.items.len();
        }
        let i = match self.state.selected() {
            Some(i) => {
                if i <= page_size {
                    self.items.len() - page_size
                } else {
                    i - page_size
                }
            }
            None => 0,
        };
        self.state.select(Some(i));
    }

    pub fn unselect(&mut self) {
        self.state.select(None);
    }

    pub fn cur(&self) -> &T {
        let i = match self.state.selected() {
            Some(i) => i,
            None => 0,
        };
        &self.items[i]
    }
}
