//! The native menu (5, 9 and 10), built from `sonara-app`'s model of it
//! ([`sonara_app::menus`], which holds the tree, the shortcuts, the routes and the rules of the
//! items' texts and enabled states, and whose tests check them against the prototype): the
//! application menu with About Sonara, Settings… (⌘,) and Quit Sonara (⌘Q); File; Edit; Track;
//! Transport; View; Window; and Help.
//!
//! **Routes.** An item runs where its state is ([`Route`]): the project flows, the native panels
//! and the history in the desktop ([`crate::lifecycle`], [`crate::dialogs`], Undo and Redo); the
//! dialogs, the panes and tabs, and the Edit, Track and Transport items in the main window's page,
//! which has the focused editor, the selection, the playhead and the transport bar
//! (`dialog-command`, `view-command`, `menu-command`). The page runs a `menu-command` as its own
//! keys and gestures do and declines it while a dialog is open.
//!
//! **Modal.** While a native prompt or panel is open, or the page shows a dialog, the menu's items
//! do nothing, as the prototype's modal dialogs let none run (the page reports its dialogs and its
//! focused editor with [`set_menu_context`]). The system's own items (Hide, Cut, Copy, Paste,
//! Select All, …) are not Sonara's and still work.
//!
//! The predefined Quit item would end the process at once (`terminate:`, which the windowing
//! library does not let the application refuse), so Quit is Sonara's own item.
//!
//! **Live items.** Undo and Redo read "Undo label" and "Redo label" and are enabled from the
//! history; Open Recent lists the recent projects, or a disabled "No recent projects".
//! The windowing library has no "about to show" event, so the desktop refreshes them on
//! every session event ([`MenuHandles`]). The Edit menu's clip items follow the page's focused
//! editor. The pane items are checked as the page shows the panes.
//!
//! **Keys.** The web view hands every key to the page before the menu sees it: the focused editor
//! takes its own ⌘D, ⌘E, ⌘A and Delete, a text field its typed keys, and the page runs the
//! transport's single-key shortcuts itself; an accelerator fires only for a key the page left
//! alone.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, PoisonError};

use serde::{Deserialize, Serialize};
use sonara_app::chrome::WindowChrome;
use sonara_app::menus::{
    self, EditFocus, Entry, ItemState, MenuAction, PageCommand, Route, SystemItem,
};
use sonara_app::window_layout::PaneLayout;
use tauri::menu::{
    CheckMenuItem, CheckMenuItemBuilder, Menu, MenuBuilder, MenuEvent, MenuItem, MenuItemBuilder,
    Submenu, SubmenuBuilder,
};
use tauri::{AppHandle, Emitter, Manager, State, Wry};

use crate::lifecycle;
use crate::shell::Shell;
use crate::window::MAIN;

/// The menu items that follow the session and the page.
pub struct MenuHandles {
    undo: MenuItem<Wry>,
    redo: MenuItem<Wry>,
    /// The Edit menu's Split, Duplicate and Delete items.
    edit: Vec<(MenuAction, MenuItem<Wry>)>,
    recent: Submenu<Wry>,
    /// The paths of the Open Recent items, in order.
    recent_paths: Mutex<Vec<PathBuf>>,
    browser: CheckMenuItem<Wry>,
    inspector: CheckMenuItem<Wry>,
    editors: CheckMenuItem<Wry>,
    /// What the page last reported.
    context: Mutex<MenuContext>,
}

/// What the main window's page reports for the menu: its focused editor and whether it shows a
/// dialog.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct MenuContext {
    /// The editor with the keyboard focus.
    pub focus: EditFocus,
    /// Whether a dialog is open.
    pub modal: bool,
}

/// What the page is asked to do from the View menu (`view-command`).
#[derive(Debug, Clone, Serialize)]
pub struct ViewCommand {
    /// `toggle-browser`, `toggle-inspector`, `toggle-editors`, `mixer`, `piano-roll` or
    /// `devices`.
    pub command: String,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The items [`build`] keeps hold of.
#[derive(Default)]
struct Kept {
    items: Vec<(MenuAction, MenuItem<Wry>)>,
    checks: Vec<(MenuAction, CheckMenuItem<Wry>)>,
    recent: Option<Submenu<Wry>>,
}

impl Kept {
    fn item(&self, action: MenuAction) -> Option<MenuItem<Wry>> {
        self.items
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, item)| item.clone())
    }

    fn check(&self, action: MenuAction) -> Option<CheckMenuItem<Wry>> {
        self.checks
            .iter()
            .find(|(a, _)| *a == action)
            .map(|(_, item)| item.clone())
    }
}

/// An item the menu's model lacks (sonara-app's tests check that it has them all).
fn missing(what: &str) -> tauri::Error {
    tauri::Error::Io(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        format!("the menu's model has no {what}"),
    ))
}

/// Builds the menu bar from [`menus::menu_bar`] and manages its [`MenuHandles`]. On macOS the
/// first submenu is the application menu.
///
/// # Errors
///
/// A Tauri error when a menu item cannot be created.
pub fn build(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
    let mut kept = Kept::default();
    let mut bar = MenuBuilder::new(app);
    for menu in menus::menu_bar() {
        let mut submenu = SubmenuBuilder::new(app, menu.title);
        for entry in menu.entries {
            submenu = match entry {
                Entry::Item(item) => {
                    let builder = MenuItemBuilder::with_id(item.action.id(), item.text);
                    let built = match item.shortcut {
                        Some(shortcut) => builder.accelerator(shortcut.accelerator()).build(app)?,
                        None => builder.build(app)?,
                    };
                    kept.items.push((item.action, built.clone()));
                    submenu.item(&built)
                }
                Entry::Check(item) => {
                    let built = CheckMenuItemBuilder::with_id(item.action.id(), item.text)
                        .checked(true)
                        .build(app)?;
                    kept.checks.push((item.action, built.clone()));
                    submenu.item(&built)
                }
                Entry::Separator => submenu.separator(),
                Entry::System(system) => system_item(submenu, system),
                Entry::OpenRecent => {
                    let recent = SubmenuBuilder::new(app, "Open Recent").build()?;
                    kept.recent = Some(recent.clone());
                    submenu.item(&recent)
                }
            };
        }
        bar = bar.item(&submenu.build()?);
    }
    let edit = [
        MenuAction::SplitClips,
        MenuAction::DuplicateClips,
        MenuAction::DeleteClips,
    ]
    .into_iter()
    .map(|action| kept.item(action).map(|item| (action, item)))
    .collect::<Option<Vec<_>>>()
    .ok_or_else(|| missing("Edit item"))?;
    app.manage(MenuHandles {
        undo: kept.item(MenuAction::Undo).ok_or_else(|| missing("Undo"))?,
        redo: kept.item(MenuAction::Redo).ok_or_else(|| missing("Redo"))?,
        edit,
        recent: kept.recent.clone().ok_or_else(|| missing("Open Recent"))?,
        recent_paths: Mutex::new(Vec::new()),
        browser: kept
            .check(MenuAction::ToggleBrowser)
            .ok_or_else(|| missing("Browser"))?,
        inspector: kept
            .check(MenuAction::ToggleInspector)
            .ok_or_else(|| missing("Inspector"))?,
        editors: kept
            .check(MenuAction::ToggleEditors)
            .ok_or_else(|| missing("Editors"))?,
        context: Mutex::new(MenuContext::default()),
    });
    bar.build()
}

/// `submenu` with the windowing library's own item for `system`.
fn system_item(
    submenu: SubmenuBuilder<'_, Wry, AppHandle>,
    system: SystemItem,
) -> SubmenuBuilder<'_, Wry, AppHandle> {
    match system {
        SystemItem::Services => submenu.services(),
        SystemItem::Hide => submenu.hide(),
        SystemItem::HideOthers => submenu.hide_others(),
        SystemItem::ShowAll => submenu.show_all(),
        SystemItem::Cut => submenu.cut(),
        SystemItem::Copy => submenu.copy(),
        SystemItem::Paste => submenu.paste(),
        SystemItem::SelectAll => submenu.select_all(),
        SystemItem::Minimize => submenu.minimize(),
        SystemItem::Zoom => submenu.maximize(),
        SystemItem::CloseWindow => submenu.close_window(),
    }
}

impl MenuHandles {
    /// Undo and Redo as `chrome` words them.
    ///
    /// # Errors
    ///
    /// A Tauri error when an item cannot be changed.
    pub fn show_history(&self, chrome: &WindowChrome) -> tauri::Result<()> {
        self.undo.set_text(&chrome.undo)?;
        self.undo.set_enabled(chrome.can_undo)?;
        self.redo.set_text(&chrome.redo)?;
        self.redo.set_enabled(chrome.can_redo)
    }

    /// Open Recent with `paths`: one item per project, named by its file stem, or
    /// the disabled "No recent projects".
    ///
    /// # Errors
    ///
    /// A Tauri error when the submenu cannot be changed.
    pub fn show_recent(&self, app: &AppHandle, paths: Vec<String>) -> tauri::Result<()> {
        let items = menus::recent_items(&paths);
        let paths: Vec<PathBuf> = paths.into_iter().map(PathBuf::from).collect();
        let mut shown = lock(&self.recent_paths);
        if *shown == paths && !self.recent.items()?.is_empty() {
            return Ok(());
        }
        for item in self.recent.items()? {
            self.recent.remove(&item)?;
        }
        for item in items {
            let entry = MenuItemBuilder::with_id(item.id, item.text)
                .enabled(item.enabled)
                .build(app)?;
            self.recent.append(&entry)?;
        }
        *shown = paths;
        Ok(())
    }

    /// The pane items checked as `panes` shows them.
    ///
    /// # Errors
    ///
    /// A Tauri error when an item cannot be changed.
    pub fn show_panes(&self, panes: &PaneLayout) -> tauri::Result<()> {
        self.browser.set_checked(panes.browser.open)?;
        self.inspector.set_checked(panes.inspector.open)?;
        self.editors.set_checked(panes.editors.open)
    }

    /// The Edit menu's clip items as `states` word and enable them.
    fn show_edit_items(&self, states: &[ItemState]) -> tauri::Result<()> {
        for state in states {
            if let Some((_, item)) = self.edit.iter().find(|(action, _)| *action == state.action) {
                item.set_text(state.text)?;
                item.set_enabled(state.enabled)?;
            }
        }
        Ok(())
    }

    /// Takes the page's `context`; the Edit menu follows a new focused editor.
    fn set_context(&self, context: MenuContext) -> tauri::Result<()> {
        let before = std::mem::replace(&mut *lock(&self.context), context);
        if before.focus == context.focus {
            return Ok(());
        }
        self.show_edit_items(&menus::edit_items(context.focus))
    }

    fn is_modal(&self) -> bool {
        lock(&self.context).modal
    }

    fn recent_path(&self, index: usize) -> Option<PathBuf> {
        lock(&self.recent_paths).get(index).cloned()
    }
}

/// The main window's page reports its focused editor and whether it shows a dialog (the
/// modal menu). Runs on the main thread, which changes the menu's items.
///
/// # Errors
///
/// An Edit item that cannot be changed.
#[tauri::command]
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri passes command arguments by value"
)]
pub fn set_menu_context(menu: State<'_, MenuHandles>, context: MenuContext) -> Result<(), String> {
    menu.set_context(context)
        .map_err(|error| format!("cannot update the Edit menu: {error}"))
}

/// The page's dialog of a [`Route::PageDialog`] action.
fn dialog_of(action: MenuAction) -> Option<&'static str> {
    Some(match action {
        MenuAction::About => "about",
        MenuAction::Settings => "settings",
        MenuAction::NewProject => "new-project",
        MenuAction::ImportAudio => "import-audio",
        MenuAction::ExportAudio => "export-audio",
        MenuAction::AiComposer => "ai-composer",
        MenuAction::AgentConnection => "agent-connection",
        _ => return None,
    })
}

/// Handles a menu click or accelerator.
#[expect(
    clippy::needless_pass_by_value,
    reason = "Tauri's menu event handler signature"
)]
pub fn handle(app: &AppHandle, event: MenuEvent) {
    let id = event.id().as_ref();
    let prompting = app.state::<Shell>().is_prompting();
    let modal = app
        .try_state::<MenuHandles>()
        .is_some_and(|menu| menu.is_modal());
    #[cfg(feature = "bench")]
    crate::bench::menu_handled(app, id, prompting || modal);
    if prompting || modal {
        log::info!("menu {id}: a prompt or a dialog is open");
        return;
    }
    if let Some(index) = menus::recent_index(id) {
        let path = app.state::<MenuHandles>().recent_path(index);
        if let Some(path) = path {
            lifecycle::open_project(app, Some(path));
        }
        return;
    }
    let Some(action) = MenuAction::from_id(id) else {
        log::debug!("menu {id}: not Sonara's");
        return;
    };
    match action.route() {
        Route::PageCommand => {
            if let Some(command) = PageCommand::of(action)
                && let Err(error) = app.emit_to(MAIN, "menu-command", &command)
            {
                log::warn!("cannot send the menu command {id}: {error}");
            }
        }
        Route::PageView => {
            // The check items toggle themselves; the page decides and reports back.
            let command = ViewCommand {
                command: id.trim_start_matches("view:").to_owned(),
            };
            if let Err(error) = app.emit("view-command", &command) {
                log::warn!("cannot send the view command: {error}");
            }
        }
        Route::PageDialog => {
            if let Some(dialog) = dialog_of(action) {
                crate::dialogs::open(app, dialog);
            }
        }
        Route::Desktop => run_in_desktop(app, action),
    }
}

/// The items that run in the desktop.
fn run_in_desktop(app: &AppHandle, action: MenuAction) {
    match action {
        MenuAction::Quit => crate::quit::request(app),
        MenuAction::Open => lifecycle::open_project(app, None),
        MenuAction::Save => lifecycle::save_from_menu(app),
        MenuAction::SaveAs => lifecycle::save_as_from_menu(app),
        MenuAction::SavePackagedCopy => crate::dialogs::save_packaged_copy(app),
        MenuAction::ImportMidi => crate::dialogs::import_midi(app),
        MenuAction::ExportMidi => crate::dialogs::export_midi(app),
        MenuAction::VerifyProject => crate::dialogs::verify_project(app),
        MenuAction::EmbedLinkedMedia => crate::dialogs::embed_linked_media(app),
        MenuAction::RemoveUnusedMedia => crate::dialogs::remove_unused_media(app),
        MenuAction::Undo => crate::commands::step_from_menu(app, false),
        MenuAction::Redo => crate::commands::step_from_menu(app, true),
        other => log::warn!("menu {}: not the desktop's", other.id()),
    }
}
