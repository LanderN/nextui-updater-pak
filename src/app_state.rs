use std::sync::Arc;

use parking_lot::Mutex;

use crate::github::{Release, ReleaseAndTag, Tag};

// Application state shared between UI thread and update thread
#[derive(Clone)]
pub enum Progress {
    Indeterminate,
    Determinate(f32),
}

pub struct AppState {
    submenu: Submenu,
    current_version: Option<String>,
    nextui_release: Option<Release>,
    nextui_tag: Option<Tag>,
    nextui_releases_and_tags: Option<Vec<ReleaseAndTag>>,
    nextui_releases_and_tags_index: Option<usize>,
    release_selection_menu: bool,
    release_selection_confirmed: bool,
    current_operation: Option<String>,
    progress: Option<Progress>,
    error: Option<String>,
    hint: Option<String>,
    should_quit: bool,
}

#[derive(Clone, Copy)]
pub enum Submenu {
    NextUI,
}

pub struct AppStateManager {
    state: Arc<Mutex<AppState>>,
}

impl AppStateManager {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(AppState {
                submenu: Submenu::NextUI,
                current_version: None,
                nextui_release: None,
                nextui_tag: None,
                nextui_releases_and_tags: None,
                nextui_releases_and_tags_index: None,
                release_selection_menu: false,
                release_selection_confirmed: false,
                current_operation: None,
                progress: None,
                error: None,
                hint: None,
                should_quit: false,
            })),
        }
    }

    // Method to clone the inner Arc
    pub fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
        }
    }

    // Getter methods
    pub fn submenu(&self) -> Submenu {
        self.state.lock().submenu
    }

    pub fn should_quit(&self) -> bool {
        self.state.lock().should_quit
    }

    pub fn current_operation(&self) -> Option<String> {
        self.state.lock().current_operation.clone()
    }

    pub fn progress(&self) -> Option<Progress> {
        self.state.lock().progress.clone()
    }

    pub fn error(&self) -> Option<String> {
        self.state.lock().error.clone()
    }

    pub fn hint(&self) -> Option<String> {
        self.state.lock().hint.clone()
    }

    pub fn current_version(&self) -> Option<String> {
        self.state.lock().current_version.clone()
    }

    pub fn nextui_release(&self) -> Option<Release> {
        self.state.lock().nextui_release.clone()
    }

    pub fn nextui_tag(&self) -> Option<Tag> {
        self.state.lock().nextui_tag.clone()
    }

    pub fn nextui_releases_and_tags(&self) -> Option<Vec<ReleaseAndTag>> {
        self.state.lock().nextui_releases_and_tags.clone()
    }

    pub fn nextui_releases_and_tags_index(&self) -> Option<usize> {
        self.state.lock().nextui_releases_and_tags_index.clone()
    }

    pub fn release_selection_menu(&self) -> bool {
        self.state.lock().release_selection_menu
    }

    pub fn release_selection_confirmed(&self) -> bool {
        self.state.lock().release_selection_confirmed
    }

    // Setter methods
    pub fn set_submenu(&self, submenu: Submenu) {
        self.state.lock().submenu = submenu;
    }

    pub fn set_should_quit(&self, should_quit: bool) {
        self.state.lock().should_quit = should_quit;
    }

    pub fn set_current_operation(&self, operation: Option<String>) {
        self.state.lock().current_operation = operation;
    }

    pub fn set_progress(&self, progress: Option<Progress>) {
        self.state.lock().progress = progress;
    }

    pub fn set_error(&self, error: Option<String>) {
        self.state.lock().error = error;
    }

    pub fn set_hint(&self, hint: Option<String>) {
        self.state.lock().hint = hint;
    }

    pub fn set_current_version(&self, version: Option<String>) {
        self.state.lock().current_version = version;
    }

    pub fn set_nextui_release(&self, release: Option<Release>) {
        self.state.lock().nextui_release = release;
    }

    pub fn set_nextui_tag(&self, tag: Option<Tag>) {
        self.state.lock().nextui_tag = tag;
    }

    pub fn set_nextui_releases_and_tags(&self, releases_and_tags: Option<Vec<ReleaseAndTag>>) {
        self.state.lock().nextui_releases_and_tags = releases_and_tags;
    }

    pub fn set_nextui_releases_and_tags_index(&self, releases_and_tags_index: Option<usize>) {
        self.state.lock().nextui_releases_and_tags_index = releases_and_tags_index;
    }

    pub fn set_release_selection_menu(&self, release_selection_menu: bool) {
        self.state.lock().release_selection_menu = release_selection_menu;
    }

    pub fn set_release_selection_confirmed(&self, release_selection_confirmed: bool) {
        self.state.lock().release_selection_confirmed = release_selection_confirmed;
    }

    // Combined operations
    pub fn start_operation(&self, operation: &str) {
        let mut state = self.state.lock();
        state.current_operation = Some(operation.to_string());
        state.progress = Some(Progress::Indeterminate);
    }

    pub fn start_determinate_operation(&self, operation: &str) {
        let mut state = self.state.lock();
        state.current_operation = Some(operation.to_string());
        state.progress = Some(Progress::Determinate(0.0));
    }

    pub fn update_progress(&self, progress: f32) {
        self.state.lock().progress = Some(Progress::Determinate(progress));
    }

    pub fn finish_operation(&self) {
        let mut state = self.state.lock();
        state.current_operation = None;
        state.progress = None;
    }

    pub fn set_operation_failed(&self, error_msg: &str) {
        let mut state = self.state.lock();
        state.current_operation = None;
        state.error = Some(error_msg.to_string());
        state.progress = None;
    }

    pub fn clear_error(&self) {
        self.state.lock().error = None;
    }

    pub fn enter_submenu(&self, submenu: Submenu) {
        let mut state = self.state.lock();
        state.submenu = submenu;
        state.hint = None;
    }

    // Access to inner Arc<Mutex<AppState>> when necessary
    pub fn inner(&self) -> Arc<Mutex<AppState>> {
        Arc::clone(&self.state)
    }
}
