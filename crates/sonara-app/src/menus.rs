//! The native menu bar of the desktop application: the prototype's `MainWindow._build_menus` tree as data, with its macOS placement,
//! its shortcuts, and the rules for the items' texts and enabled states. The desktop builds the
//! menu from [`menu_bar`] and dispatches each item by its [`MenuAction`]; the container's tests
//! check the tree against the prototype's.
//!
//! **Placement (macOS).** As Qt does on macOS, About Sonara, Settings… and Quit move from Help and
//! File into the application menu ("Sonara"), where Quit reads "Quit Sonara". The system's own
//! items ([`SystemItem`]) are not the prototype's: the application menu's Services, Hide, Hide
//! Others and Show All; the Edit menu's Cut, Copy, Paste and Select All, through which the web
//! view edits the text of its fields; and the Window menu.
//!
//! **Enabled states and texts** are the prototype's. Undo and Redo follow the history and read
//! "Undo label" ([`crate::chrome::WindowChrome`]). Open Recent lists the recent projects, or a
//! disabled "No recent projects" ([`recent_items`]). Every other item is always enabled and does
//! nothing when it has nothing to act on, except the Edit menu's three clip items ([`edit_items`]).
//!
//! **G9**. The prototype's Edit menu took ⌘D and ⌘E window-wide, so the focused
//! editor never saw them: in the timeline, ⌘D duplicated each clip right after itself as separate
//! steps and ⌘E split only at the playhead, as separate steps; in the piano roll, ⌘D duplicated the
//! selected clips instead of the notes. In Sonara the web view gives the page a key before the menu
//! sees it, so the focused editor handles its own ⌘D, ⌘E, ⌘A and Delete, and the Edit menu's items
//! act on the focused editor ([`EditFocus`]): the timeline's own duplicate ("Duplicate Clips", one
//! step after the selection's span), split and delete; the piano roll's notes ("Duplicate Notes",
//! "Delete Notes"; it splits nothing); nothing while a device card has focus (its own keys are
//! `ValueControl`'s). With no editor focused they act on the timeline's clips, as the prototype's
//! did from anywhere.
//!
//! **Single-key shortcuts.** The Transport menu shows Space, Home, R, L and C, the prototype's
//! application-wide shortcuts. The page runs them itself, never while a text field has focus or a
//! dialog is open; a text field takes its typed keys before the menu could, and the menu's item
//! runs only for a key the page left alone, which the page declines while a dialog is open.

use serde::Serialize;

/// What a menu item does. The desktop handles each one by its [`MenuAction::id`], the item's ID
/// in the native menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(into = "&'static str")]
pub enum MenuAction {
    /// The About dialog.
    About,
    /// The Settings dialog.
    Settings,
    /// Quit after the unsaved-changes prompt.
    Quit,
    /// The New Project dialog.
    NewProject,
    /// The open panel.
    Open,
    /// MW-04.
    Save,
    /// MW-05.
    SaveAs,
    /// MW-06.
    SavePackagedCopy,
    /// MW-07.
    ImportAudio,
    /// MW-08.
    ImportMidi,
    /// The Export dialog.
    ExportAudio,
    /// MW-10.
    ExportMidi,
    /// MW-11.
    VerifyProject,
    /// MW-12.
    EmbedLinkedMedia,
    /// MW-13.
    RemoveUnusedMedia,
    /// MW-16.
    Undo,
    /// MW-16.
    Redo,
    /// MW-17 (G9: the focused editor's split).
    SplitClips,
    /// MW-18 (G9: the focused editor's duplicate).
    DuplicateClips,
    /// MW-19 (G9: the focused editor's delete).
    DeleteClips,
    /// MW-20.
    NewInstrumentTrack,
    /// MW-20.
    NewDrumTrack,
    /// MW-20.
    NewAudioTrack,
    /// MW-20.
    NewBus,
    /// MW-20.
    NewFolder,
    /// MW-21.
    DeleteSelectedTrack,
    /// TB-01.
    PlayStop,
    /// TB-02.
    ReturnToStart,
    /// TB-04.
    Record,
    /// TB-05.
    Loop,
    /// TB-06.
    Metronome,
    /// Shows or hides the Browser pane.
    ToggleBrowser,
    /// Shows or hides the Inspector pane.
    ToggleInspector,
    /// Shows or hides the Editors pane.
    ToggleEditors,
    /// Raises the Mixer tab.
    ShowMixer,
    /// Raises the Piano Roll tab.
    ShowPianoRoll,
    /// Raises the Devices tab.
    ShowDevices,
    /// MW-23, DL-03.
    AgentConnection,
}

impl MenuAction {
    /// Every action, in the menu's order.
    pub const ALL: [Self; 38] = [
        Self::About,
        Self::Settings,
        Self::Quit,
        Self::NewProject,
        Self::Open,
        Self::Save,
        Self::SaveAs,
        Self::SavePackagedCopy,
        Self::ImportAudio,
        Self::ImportMidi,
        Self::ExportAudio,
        Self::ExportMidi,
        Self::VerifyProject,
        Self::EmbedLinkedMedia,
        Self::RemoveUnusedMedia,
        Self::Undo,
        Self::Redo,
        Self::SplitClips,
        Self::DuplicateClips,
        Self::DeleteClips,
        Self::NewInstrumentTrack,
        Self::NewDrumTrack,
        Self::NewAudioTrack,
        Self::NewBus,
        Self::NewFolder,
        Self::DeleteSelectedTrack,
        Self::PlayStop,
        Self::ReturnToStart,
        Self::Record,
        Self::Loop,
        Self::Metronome,
        Self::ToggleBrowser,
        Self::ToggleInspector,
        Self::ToggleEditors,
        Self::ShowMixer,
        Self::ShowPianoRoll,
        Self::ShowDevices,
        Self::AgentConnection,
    ];

    /// The item's ID in the native menu. The View items' IDs carry the page's `view-command`
    /// after `view:`.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::About => "about",
            Self::Settings => "settings",
            Self::Quit => "quit",
            Self::NewProject => "new-project",
            Self::Open => "open",
            Self::Save => "save",
            Self::SaveAs => "save-as",
            Self::SavePackagedCopy => "save-packaged",
            Self::ImportAudio => "import-audio",
            Self::ImportMidi => "import-midi",
            Self::ExportAudio => "export-audio",
            Self::ExportMidi => "export-midi",
            Self::VerifyProject => "verify",
            Self::EmbedLinkedMedia => "embed-media",
            Self::RemoveUnusedMedia => "remove-unused-media",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::SplitClips => "split-clips",
            Self::DuplicateClips => "duplicate-clips",
            Self::DeleteClips => "delete-clips",
            Self::NewInstrumentTrack => "new-instrument-track",
            Self::NewDrumTrack => "new-drum-track",
            Self::NewAudioTrack => "new-audio-track",
            Self::NewBus => "new-bus",
            Self::NewFolder => "new-folder",
            Self::DeleteSelectedTrack => "delete-track",
            Self::PlayStop => "play-stop",
            Self::ReturnToStart => "return-to-start",
            Self::Record => "record",
            Self::Loop => "loop",
            Self::Metronome => "metronome",
            Self::ToggleBrowser => "view:toggle-browser",
            Self::ToggleInspector => "view:toggle-inspector",
            Self::ToggleEditors => "view:toggle-editors",
            Self::ShowMixer => "view:mixer",
            Self::ShowPianoRoll => "view:piano-roll",
            Self::ShowDevices => "view:devices",
            Self::AgentConnection => "agent-connection",
        }
    }

    /// The action of the item `id`; `None` for an Open Recent entry or a system item.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }

    /// Where the action runs: the actions that need the page's state (the focused editor, the
    /// selection, the playhead, the transport bar) or its panes and dialogs go to the main
    /// window's page; the others run in the desktop.
    #[must_use]
    pub const fn route(self) -> Route {
        match self {
            Self::SplitClips
            | Self::DuplicateClips
            | Self::DeleteClips
            | Self::NewInstrumentTrack
            | Self::NewDrumTrack
            | Self::NewAudioTrack
            | Self::NewBus
            | Self::NewFolder
            | Self::DeleteSelectedTrack
            | Self::PlayStop
            | Self::ReturnToStart
            | Self::Record
            | Self::Loop
            | Self::Metronome => Route::PageCommand,
            Self::ToggleBrowser
            | Self::ToggleInspector
            | Self::ToggleEditors
            | Self::ShowMixer
            | Self::ShowPianoRoll
            | Self::ShowDevices => Route::PageView,
            Self::About
            | Self::Settings
            | Self::NewProject
            | Self::ImportAudio
            | Self::ExportAudio
            | Self::AgentConnection => Route::PageDialog,
            Self::Quit
            | Self::Open
            | Self::Save
            | Self::SaveAs
            | Self::SavePackagedCopy
            | Self::ImportMidi
            | Self::ExportMidi
            | Self::VerifyProject
            | Self::EmbedLinkedMedia
            | Self::RemoveUnusedMedia
            | Self::Undo
            | Self::Redo => Route::Desktop,
        }
    }
}

impl From<MenuAction> for &'static str {
    fn from(action: MenuAction) -> Self {
        action.id()
    }
}

/// Where a [`MenuAction`] runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// In the desktop: a project flow, a native panel or the history.
    Desktop,
    /// The main window's page runs it (`menu-command`, [`PageCommand`]).
    PageCommand,
    /// The main window's page shows or hides a pane or raises a tab (`view-command`).
    PageView,
    /// The main window's page opens a dialog (`dialog-command`).
    PageDialog,
}

/// What the main window's page is asked to do from the Edit, Track and Transport menus
/// (`menu-command`). It runs it as the page's own keys and gestures do, through the edit
/// boundary with the source "gui", and declines it while a dialog is open.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PageCommand {
    /// The action's ID ([`MenuAction::id`]).
    pub command: &'static str,
    /// For a new track: the edit that creates it, which the page runs and whose track it
    /// then selects.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub edit: Option<NewTrackEdit>,
}

impl PageCommand {
    /// The page's command of `action`; `None` for an action that does not go to the page this
    /// way.
    #[must_use]
    pub fn of(action: MenuAction) -> Option<Self> {
        (action.route() == Route::PageCommand).then(|| Self {
            command: action.id(),
            edit: new_track(action),
        })
    }
}

/// MW-20's new track, as the edit boundary's `create_track` reads it (`sonara_app::edit::Edit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NewTrackEdit {
    /// Always `create_track`.
    pub op: &'static str,
    /// The prototype's `add_track` names: "Instrument", "Drums", "Audio", "Bus" or "Folder".
    pub name: &'static str,
    /// `instrument`, `audio`, `bus` or `folder`.
    pub kind: &'static str,
    /// The instrument of an instrument track: the poly synth, or the drum kit for New Drum Track.
    pub instrument: Option<&'static str>,
}

/// The poly synth, an instrument track's instrument.
const POLY_SYNTH: &str = "sonara.synth.poly";
/// The drum kit (`catalog::drum_map`'s plugin).
const DRUM_KIT: &str = "sonara.drums";

/// The track the Track menu's `action` creates, as `MainWindow.add_track` names it; `None`
/// for any other action.
#[must_use]
pub const fn new_track(action: MenuAction) -> Option<NewTrackEdit> {
    let (name, kind, instrument) = match action {
        MenuAction::NewInstrumentTrack => ("Instrument", "instrument", Some(POLY_SYNTH)),
        MenuAction::NewDrumTrack => ("Drums", "instrument", Some(DRUM_KIT)),
        MenuAction::NewAudioTrack => ("Audio", "audio", None),
        MenuAction::NewBus => ("Bus", "bus", None),
        MenuAction::NewFolder => ("Folder", "folder", None),
        _ => return None,
    };
    Some(NewTrackEdit {
        op: "create_track",
        name,
        kind,
        instrument,
    })
}

/// A keyboard shortcut of a menu item, in macOS terms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    /// With ⌘.
    pub command: bool,
    /// With ⇧.
    pub shift: bool,
    /// The key: a letter or digit as it is printed, `,`, or a named key (`Space`, `Home`).
    pub key: &'static str,
}

impl Shortcut {
    const fn command(key: &'static str) -> Self {
        Self {
            command: true,
            shift: false,
            key,
        }
    }

    const fn command_shift(key: &'static str) -> Self {
        Self {
            command: true,
            shift: true,
            key,
        }
    }

    const fn key(key: &'static str) -> Self {
        Self {
            command: false,
            shift: false,
            key,
        }
    }

    /// The accelerator as the desktop's menu library parses it: "CmdOrCtrl+Shift+S", "Space".
    #[must_use]
    pub fn accelerator(&self) -> String {
        let mut text = String::new();
        if self.command {
            text.push_str("CmdOrCtrl+");
        }
        if self.shift {
            text.push_str("Shift+");
        }
        text.push_str(self.key);
        text
    }

    /// As a macOS menu prints it, and as writes it: "⇧⌘S", "⌘,", "Space".
    #[must_use]
    pub fn glyphs(&self) -> String {
        let mut text = String::new();
        if self.shift {
            text.push('⇧');
        }
        if self.command {
            text.push('⌘');
        }
        text.push_str(self.key);
        text
    }
}

/// A menu item with its action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item {
    /// What it does.
    pub action: MenuAction,
    /// Its text when the menu is built (Undo, Redo and the Edit menu's clip items change theirs).
    pub text: &'static str,
    /// Its shortcut.
    pub shortcut: Option<Shortcut>,
}

/// A system item of macOS, which the windowing library provides and handles itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SystemItem {
    /// The Services submenu.
    Services,
    /// Hide Sonara (⌘H).
    Hide,
    /// Hide Others (⌥⌘H).
    HideOthers,
    /// Show All.
    ShowAll,
    /// Cut (⌘X), for text fields.
    Cut,
    /// Copy (⌘C), for text fields.
    Copy,
    /// Paste (⌘V), for text fields.
    Paste,
    /// Select All (⌘A), for text fields; an editor with focus takes ⌘A first.
    SelectAll,
    /// Minimize (⌘M).
    Minimize,
    /// Zoom.
    Zoom,
    /// Close Window (⌘W), which asks about unsaved changes as Quit does.
    CloseWindow,
}

/// An entry of a menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entry {
    /// An item.
    Item(Item),
    /// A checkable item (the View menu's panes), checked as the page shows the pane.
    Check(Item),
    /// A separator.
    Separator,
    /// A system item.
    System(SystemItem),
    /// File ▸ Open Recent, filled from the recent projects ([`recent_items`]).
    OpenRecent,
}

/// One menu of the menu bar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Menu {
    /// Its title in the menu bar; the first menu is the application menu.
    pub title: &'static str,
    /// Its entries in order.
    pub entries: Vec<Entry>,
}

const fn item(action: MenuAction, text: &'static str) -> Entry {
    Entry::Item(Item {
        action,
        text,
        shortcut: None,
    })
}

const fn keyed(action: MenuAction, text: &'static str, shortcut: Shortcut) -> Entry {
    Entry::Item(Item {
        action,
        text,
        shortcut: Some(shortcut),
    })
}

const fn check(action: MenuAction, text: &'static str) -> Entry {
    Entry::Check(Item {
        action,
        text,
        shortcut: None,
    })
}

const fn cmd(key: &'static str) -> Shortcut {
    Shortcut::command(key)
}

const fn cmd_shift(key: &'static str) -> Shortcut {
    Shortcut::command_shift(key)
}

const fn plain(key: &'static str) -> Shortcut {
    Shortcut::key(key)
}

/// The menu bar: the prototype's File, Edit, Track, Transport, View and Help menus,
/// with About Sonara, Settings… and Quit in the application menu, and the system's items.
#[must_use]
pub fn menu_bar() -> Vec<Menu> {
    vec![
        application_menu(),
        file_menu(),
        edit_menu(),
        track_menu(),
        transport_menu(),
        view_menu(),
        Menu {
            title: "Window",
            entries: vec![
                Entry::System(SystemItem::Minimize),
                Entry::System(SystemItem::Zoom),
                Entry::Separator,
                Entry::System(SystemItem::CloseWindow),
            ],
        },
        Menu {
            title: "Help",
            entries: vec![item(MenuAction::AgentConnection, "Agent Connection…")],
        },
    ]
}

fn application_menu() -> Menu {
    use Entry::{Separator, System};
    Menu {
        title: "Sonara",
        entries: vec![
            item(MenuAction::About, "About Sonara"),
            Separator,
            keyed(MenuAction::Settings, "Settings…", cmd(",")),
            Separator,
            System(SystemItem::Services),
            Separator,
            System(SystemItem::Hide),
            System(SystemItem::HideOthers),
            System(SystemItem::ShowAll),
            Separator,
            keyed(MenuAction::Quit, "Quit Sonara", cmd("Q")),
        ],
    }
}

fn file_menu() -> Menu {
    use Entry::{OpenRecent, Separator};
    use MenuAction as A;
    Menu {
        title: "File",
        entries: vec![
            keyed(A::NewProject, "New Project…", cmd("N")),
            keyed(A::Open, "Open…", cmd("O")),
            OpenRecent,
            Separator,
            keyed(A::Save, "Save", cmd("S")),
            keyed(A::SaveAs, "Save As…", cmd_shift("S")),
            item(A::SavePackagedCopy, "Save Packaged Copy (.sonara archive)…"),
            Separator,
            keyed(A::ImportAudio, "Import Audio…", cmd("I")),
            keyed(A::ImportMidi, "Import MIDI…", cmd_shift("I")),
            keyed(A::ExportAudio, "Export Audio…", cmd_shift("E")),
            item(A::ExportMidi, "Export MIDI…"),
            Separator,
            item(A::VerifyProject, "Verify Project"),
            item(A::EmbedLinkedMedia, "Embed Linked Media"),
            item(A::RemoveUnusedMedia, "Remove Unused Media…"),
        ],
    }
}

fn edit_menu() -> Menu {
    use Entry::{Separator, System};
    use MenuAction as A;
    Menu {
        title: "Edit",
        entries: vec![
            keyed(A::Undo, "Undo", cmd("Z")),
            keyed(A::Redo, "Redo", cmd_shift("Z")),
            Separator,
            keyed(A::SplitClips, SPLIT_CLIPS, cmd("E")),
            keyed(A::DuplicateClips, DUPLICATE_CLIPS, cmd("D")),
            item(A::DeleteClips, DELETE_CLIPS),
            Separator,
            System(SystemItem::Cut),
            System(SystemItem::Copy),
            System(SystemItem::Paste),
            System(SystemItem::SelectAll),
        ],
    }
}

fn track_menu() -> Menu {
    use MenuAction as A;
    Menu {
        title: "Track",
        entries: vec![
            keyed(A::NewInstrumentTrack, "New Instrument Track", cmd("T")),
            item(A::NewDrumTrack, "New Drum Track"),
            keyed(A::NewAudioTrack, "New Audio Track", cmd_shift("T")),
            item(A::NewBus, "New Bus"),
            item(A::NewFolder, "New Folder"),
            Entry::Separator,
            item(A::DeleteSelectedTrack, "Delete Selected Track"),
        ],
    }
}

fn transport_menu() -> Menu {
    use MenuAction as A;
    Menu {
        title: "Transport",
        entries: vec![
            keyed(A::PlayStop, "Play / Stop", plain("Space")),
            keyed(A::ReturnToStart, "Return to Start", plain("Home")),
            keyed(A::Record, "Record", plain("R")),
            keyed(A::Loop, "Loop", plain("L")),
            keyed(A::Metronome, "Metronome", plain("C")),
        ],
    }
}

fn view_menu() -> Menu {
    use MenuAction as A;
    Menu {
        title: "View",
        entries: vec![
            check(A::ToggleBrowser, "Browser"),
            check(A::ToggleInspector, "Inspector"),
            check(A::ToggleEditors, "Editors"),
            Entry::Separator,
            keyed(A::ShowMixer, "Mixer", cmd("1")),
            keyed(A::ShowPianoRoll, "Piano Roll", cmd("2")),
            keyed(A::ShowDevices, "Devices", cmd("3")),
        ],
    }
}

const SPLIT_CLIPS: &str = "Split Clips at Playhead";
const DUPLICATE_CLIPS: &str = "Duplicate Clips";
const DELETE_CLIPS: &str = "Delete Clips";

/// The editor whose element has the keyboard focus in the main window's page, which the Edit
/// menu's clip items act on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum EditFocus {
    /// No editor: the focus is elsewhere (a transport button, the inspector, the browser) or
    /// nowhere. The items act on the timeline's clips, as the prototype's did.
    #[default]
    None,
    /// The timeline: its toolbar, ruler, track headers or arrangement.
    Timeline,
    /// The piano roll.
    PianoRoll,
    /// The Devices tab's cards.
    Devices,
}

/// A menu item's text and enabled state of one moment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ItemState {
    /// The item.
    pub action: MenuAction,
    /// Its text.
    pub text: &'static str,
    /// Whether it can be chosen.
    pub enabled: bool,
}

const fn state(action: MenuAction, text: &'static str, enabled: bool) -> ItemState {
    ItemState {
        action,
        text,
        enabled,
    }
}

/// The Edit menu's Split, Duplicate and Delete items while `focus` has the keyboard focus.
#[must_use]
pub const fn edit_items(focus: EditFocus) -> [ItemState; 3] {
    match focus {
        EditFocus::None | EditFocus::Timeline => [
            state(MenuAction::SplitClips, SPLIT_CLIPS, true),
            state(MenuAction::DuplicateClips, DUPLICATE_CLIPS, true),
            state(MenuAction::DeleteClips, DELETE_CLIPS, true),
        ],
        // The piano roll splits nothing; its ⌘D and Delete act on the selected notes.
        EditFocus::PianoRoll => [
            state(MenuAction::SplitClips, SPLIT_CLIPS, false),
            state(MenuAction::DuplicateClips, "Duplicate Notes", true),
            state(MenuAction::DeleteClips, "Delete Notes", true),
        ],
        EditFocus::Devices => [
            state(MenuAction::SplitClips, SPLIT_CLIPS, false),
            state(MenuAction::DuplicateClips, DUPLICATE_CLIPS, false),
            state(MenuAction::DeleteClips, DELETE_CLIPS, false),
        ],
    }
}

/// An entry of File ▸ Open Recent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecentItem {
    /// The item's ID: `recent:` and its index.
    pub id: String,
    /// The project's file name without its extension, as `_fill_recent` names it.
    pub text: String,
    /// Whether it can be chosen: only "No recent projects" cannot.
    pub enabled: bool,
}

/// The prefix of an Open Recent entry's ID; its index in the recent projects follows.
pub const RECENT_PREFIX: &str = "recent:";

/// One entry per recent project, named by its file stem, or the disabled "No recent
/// projects".
#[must_use]
pub fn recent_items(paths: &[String]) -> Vec<RecentItem> {
    if paths.is_empty() {
        return vec![RecentItem {
            id: "no-recent".to_owned(),
            text: "No recent projects".to_owned(),
            enabled: false,
        }];
    }
    paths
        .iter()
        .enumerate()
        .map(|(index, path)| {
            let path = std::path::Path::new(path);
            let text = path.file_stem().map_or_else(
                || path.display().to_string(),
                |stem| stem.to_string_lossy().into_owned(),
            );
            RecentItem {
                id: format!("{RECENT_PREFIX}{index}"),
                text,
                enabled: true,
            }
        })
        .collect()
}

/// The index in the recent projects of the Open Recent entry `id`.
#[must_use]
pub fn recent_index(id: &str) -> Option<usize> {
    id.strip_prefix(RECENT_PREFIX)?.parse().ok()
}
