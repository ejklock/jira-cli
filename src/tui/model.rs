use crate::commands::MINE_JQL;
use crate::i18n::t;
use crate::models::{Issue, IssueComment, IssueRow, ProjectRow};
use crate::render::{adf_to_plain_text, adf_to_rich};

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    List,
    Projects,
    Detail,
}

/// The active list's provenance (ADR 0021 §3, BDR 0013; `Search` added by
/// ADR 0025/BDR 0016 §S2): the mine list, an ad hoc JQL search seed, or a
/// specific project's issues (`Project(key)`) drilled into from the Projects
/// screen. Determines what a Back from the List screen restores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListOrigin {
    Mine,
    Search,
    Project(String),
}

/// The footer's mode-aware hint bucket (ADR 0014 §5, BDR 0007 S7): derived
/// purely from the current screen + search-input-active + focused-link state,
/// so `footer_hint` never branches on `Model` fields directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FooterMode {
    List,
    ListSearch,
    Projects,
    Detail,
    DetailLink,
}

/// Derives the active [`FooterMode`] from the model's screen, search-input,
/// and focused-link state (BDR 0007 S7). Pure — no rendering, no I/O.
pub fn footer_mode(model: &Model) -> FooterMode {
    match model.screen {
        Screen::List if model.search.is_some() => FooterMode::ListSearch,
        Screen::List => FooterMode::List,
        Screen::Projects => FooterMode::Projects,
        Screen::Detail if model.detail_focused_link.is_some() => FooterMode::DetailLink,
        Screen::Detail => FooterMode::Detail,
    }
}

/// A transient one-line status message (BDR 0007 S8): copy-key confirmation
/// or a fetch error, rendered on the thin row above the footer and cleared by
/// the next key event.
#[derive(Debug, Clone, PartialEq)]
pub struct StatusMsg {
    pub text: String,
    pub kind: StatusKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    Info,
    Error,
}

/// The status text shown on opening the Projects screen while its fetch is
/// in flight (ADR 0021 §8, BDR 0013 S1) — the existing status seam, no
/// bespoke spinner.
const LOADING_STATUS_TEXT: &str = "Loading…";

/// A logged-in identity (email + instance name) shown in the header bar
/// (ADR 0014 §2, BDR 0007 S1). Plain data — the shell populates it from
/// already-loaded instance configuration; the domain core never fetches it.
#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    pub email: String,
    pub instance: String,
}

pub struct Model {
    pub rows: Vec<IssueRow>,
    pub selected: usize,
    pub screen: Screen,
    pub detail: Option<Issue>,
    pub detail_scroll: u16,
    pub search: Option<String>,
    pub error: Option<String>,
    pub base_url: String,
    /// The active list JQL — repeated by `LoadMore` so the next page matches
    /// the current list/search result.
    pub jql: String,
    /// The paging cursor for the current list. `None` once the last page is
    /// loaded (or before any pagination-capable fetch has completed).
    pub next_page_token: Option<String>,
    /// The description's inline link hrefs, in document order. Populated on
    /// `DetailLoaded`, cleared on `Back`.
    pub detail_links: Vec<String>,
    /// Index into `detail_links` of the currently focused link, or `None`
    /// when there are no links.
    pub detail_focused_link: Option<usize>,
    /// Logged-in identities shown in the header bar. Set once by the shell
    /// before entering the event loop; empty when none are configured.
    pub identities: Vec<Identity>,
    /// A transient status message shown above the footer (BDR 0007 S8).
    /// Cleared by `update` at the start of every key-driven `Msg`.
    pub status: Option<StatusMsg>,
    /// True while a background revalidation fetch is in flight (BDR 0008): a
    /// warm entry paints the cached snapshot immediately with this set, and
    /// `entry_cmds` dispatches the single `Cmd::RevalidateList` seam for it.
    /// Submitting a search clears it, so a late revalidation result never
    /// clobbers fresher search results (S4).
    pub revalidating: bool,
    /// The active text selection in the detail body (ADR 0019 §1), in LOGICAL
    /// coordinates only — `(logical_line, char)` pairs resolved through
    /// `compose_detail`'s extended metadata, never visual rows/columns, so
    /// scrolling never moves the selection (BDR 0011 S8). Cleared on leaving
    /// the detail screen.
    pub selection: Option<Selection>,
    /// Which list is currently loaded — the mine list, or a specific
    /// project's issues (ADR 0021 §3). Determines what a Back from the List
    /// screen restores.
    pub list_origin: ListOrigin,
    /// The projects fetched by the Projects screen (ADR 0021), retained
    /// across a Back so returning to Projects from a project's issue list
    /// shows the same rows with no refetch (BDR 0013 S4).
    pub projects: Vec<ProjectRow>,
    /// The Projects screen's selected row index, bounded to
    /// `[0, projects.len() - 1]` (BDR 0013 S2).
    pub projects_selected: usize,
    /// The open comment compose (ADR 0024 §3, BDR 0015): `Some` only while
    /// `Screen::Detail` shows its modal over the dimmed thread. `None` on
    /// every other screen and whenever no compose is open.
    pub compose: Option<Compose>,
    /// Index into the loaded issue's `comments` currently focused (ADR 0026
    /// §1, BDR 0017 S1) — a distinct axis from `detail_focused_link`, moved
    /// by `]`/`[`, clamped at the ends, `None` when the thread is empty or no
    /// comment has been focused yet. Reset to `None` on entering or leaving
    /// the Detail screen, mirroring `detail_focused_link`.
    pub detail_focused_comment: Option<usize>,
    /// The authenticated user's Cloud account id (ADR 0026 §2, BDR 0017 S2),
    /// set once from a one-shot `myself` fetch dispatched at browse startup.
    /// `None` until the fetch lands, and stays `None` on failure — safe
    /// degradation, since `is_own_comment` treats an unknown identity as
    /// owning nothing. Read only by `is_own_comment` until C4a.2 wires the
    /// edit-ownership gate — `#[allow(dead_code)]` mirrors
    /// `theme::column_header`'s bridge-to-a-later-slice pattern.
    #[allow(dead_code)]
    pub current_account_id: Option<String>,
}

impl Model {
    /// Whether `comment` was authored by the current authenticated user (ADR
    /// 0026 §2, BDR 0017 S2): `true` iff `comment.author_account_id` equals
    /// `Some(current_account_id)`. A `None` `current_account_id` (the
    /// `myself` fetch hasn't landed, or failed) means nothing is own. Gains
    /// its first production caller in C4a.2's edit-ownership gate; unused
    /// outside tests until then.
    #[allow(dead_code)]
    pub fn is_own_comment(&self, comment: &IssueComment) -> bool {
        self.current_account_id
            .as_deref()
            .is_some_and(|me| comment.author_account_id.as_deref() == Some(me))
    }
}

/// The comment compose's draft state (ADR 0024 §3, ADR 0026 §3): `buffer` is
/// the multi-line text built by typing (Enter inserts `\n`, never submits,
/// BDR 0015 S1); `status` tracks the in-box submit feedback; `target`
/// discriminates a brand-new comment from an edit of an existing one (BDR
/// 0017 S3-S4) and drives both the submit `Cmd` and the modal title.
#[derive(Debug, Clone, PartialEq)]
pub struct Compose {
    pub buffer: String,
    pub status: ComposeStatus,
    pub target: ComposeTarget,
}

/// What a compose submit does (ADR 0026 §3, BDR 0017 S3-S4): `New` posts a
/// brand-new comment (`Cmd::SubmitComment`, the C3b path, unchanged);
/// `Edit` updates the identified existing comment (`Cmd::EditComment`).
/// `Default`s to `New` so every pre-C4 compose-open path is unaffected.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum ComposeTarget {
    #[default]
    New,
    Edit {
        comment_id: String,
    },
}

impl ComposeTarget {
    /// The modal title's locale key (ADR 0026 §3, BDR 0017 S3): "New comment"
    /// for a fresh post, "Edit comment" while editing. Pure — `view.rs` runs
    /// it through `t()`.
    pub fn title_key(&self) -> &'static str {
        match self {
            ComposeTarget::New => "New comment",
            ComposeTarget::Edit { .. } => "Edit comment",
        }
    }
}

/// The compose's in-box status line (ADR 0024 §4-5, BDR 0015 S2/S4):
/// `Idle` shows no status line; `Submitting` shows the localized "Sending…"
/// while the write is in flight; `Error(reason)` preserves the draft with a
/// localized failure reason (a 401 reuses the E2 re-auth message).
#[derive(Debug, Clone, PartialEq)]
pub enum ComposeStatus {
    Idle,
    Submitting,
    Error(String),
}

/// An active detail-body text selection (ADR 0019 §1): `anchor` is where the
/// unmodified left DOWN landed, `cursor` tracks the drag; `dragged`
/// distinguishes a real drag (release copies) from a plain click (release
/// clears, copies nothing — BDR 0011 S2/S3).
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    pub anchor: (usize, usize),
    pub cursor: (usize, usize),
    pub dragged: bool,
}

/// Builds the identity header text: "{email} · {instance}" from the first
/// identity, with " (+N more)" appended when aggregating several instances
/// (ADR 0014 §2, BDR 0007 S1). Empty when no identities are set.
pub fn header_line(identities: &[Identity]) -> String {
    let Some(first) = identities.first() else {
        return String::new();
    };
    let mut line = format!("{} · {}", first.email, first.instance);
    if identities.len() > 1 {
        line.push_str(&format!(" (+{} more)", identities.len() - 1));
    }
    line
}

pub enum Msg {
    Up,
    Down,
    Quit,
    Select,
    Back,
    DetailLoaded(Box<Issue>),
    OpenSearch,
    SearchInput(char),
    SearchBackspace,
    SubmitSearch,
    CancelSearch,
    ListLoaded(Vec<IssueRow>, Option<String>),
    MoreLoaded(Vec<IssueRow>, Option<String>),
    LoadMore,
    LoadFailed(String),
    OpenLink,
    CopyKey,
    FocusNextLink,
    /// The background entry revalidation (BDR 0008 S2) completed: fresh rows
    /// + paging cursor, applied only while `revalidating` is still true.
    RevalidationLoaded(Vec<IssueRow>, Option<String>),
    /// The background entry revalidation (BDR 0008 S5) failed: the message
    /// (already mapped to the E2 re-auth guidance for a 401), surfaced only
    /// while `revalidating` is still true.
    RevalidationFailed(String),
    /// A left click resolved to a visible list card's row index (ADR 0017 §4,
    /// BDR 0009 S3): carries a plain `usize` — the mouse-event types stay in
    /// shell/view, keeping the domain core free of crossterm/ratatui (ADR
    /// 0007).
    CardClicked(usize),
    /// A Ctrl/Super-click resolved to an inline body link's href on the
    /// Detail screen (ADR 0018 §4, BDR 0010 S5): carries a plain `String` —
    /// the mouse-event/geometry types stay in shell/view (ADR 0007).
    LinkClicked(String),
    /// Unmodified left DOWN on the Detail body anchors a selection,
    /// replacing any previous one (ADR 0019 §3, BDR 0011 S1): carries a
    /// plain logical `(line, char)` — geometry stays in shell/view (ADR 0007).
    SelStart((usize, usize)),
    /// Left DRAG extends the active selection's cursor (BDR 0011 S1); a
    /// no-op with no active selection.
    SelDrag((usize, usize)),
    /// Left RELEASE on the Detail body (BDR 0011 S2/S3): `Some(text)` after a
    /// drag copies the text (existing `Cmd::CopyToClipboard` + "Copied ✓"
    /// status) and keeps the highlight; `None` (a plain click) clears the
    /// selection and copies nothing.
    SelEnd(Option<String>),
    /// `p` on the issue list (normal mode, ADR 0021 §1, BDR 0013 S1): opens
    /// the Projects screen and issues a fetch. A no-op off the List screen or
    /// with search active.
    OpenProjects,
    /// The projects fetch completed (BDR 0013 S1): the fetched rows, applied
    /// regardless of the current screen (a late result is harmless — it never
    /// changes `screen`).
    ProjectsLoaded(Vec<ProjectRow>),
    /// The projects fetch failed (BDR 0013 S5): the message (already mapped
    /// to the E2 re-auth guidance for a 401), surfaced on the status row
    /// while staying on the Projects screen.
    ProjectsFailed(String),
    /// A left click resolved to a visible Projects row's index (ADR 0021,
    /// BDR 0013 S2-S3): mirrors `CardClicked`'s contract on the List screen.
    ProjectClicked(usize),
    /// `c` on the Detail screen with a loaded issue opens the comment compose
    /// (ADR 0024 §3, BDR 0015 S1, S8): a no-op on List/Projects or with no
    /// issue loaded yet.
    OpenCompose,
    /// A printable character appended to the open compose's buffer (BDR 0015
    /// S1); a no-op with no compose open.
    ComposeInput(char),
    /// Enter inserts a newline into the open compose's buffer — never
    /// submits (BDR 0015 S1).
    ComposeNewline,
    /// Backspace deletes the open compose's last character.
    ComposeBackspace,
    /// Ctrl+S submits the open compose (BDR 0015 S2, S7): a non-empty
    /// (trimmed) buffer emits exactly one `Cmd::SubmitComment` and sets
    /// `ComposeStatus::Submitting`; an empty/whitespace buffer is a no-op and
    /// the modal stays open.
    SubmitCompose,
    /// Esc discards the open compose: no write, no refresh, the detail is
    /// unchanged (BDR 0015 S3).
    CancelCompose,
    /// The comment write succeeded (ADR 0024 §5, BDR 0015 S2): the compose
    /// closes and the thread is refreshed from the server — never a
    /// locally-fabricated comment.
    CommentMutationOk,
    /// The comment write failed (ADR 0024 §5, BDR 0015 S4): the buffer is
    /// preserved, the compose shows the localized reason (a 401 reuses the
    /// E2 re-auth message), and no refresh happens.
    CommentMutationErr(String),
    /// `]` advances the focused comment (ADR 0026 §1, BDR 0017 S1): a no-op
    /// off `Screen::Detail`, with no loaded issue, or an empty thread.
    FocusNextComment,
    /// `[` retreats the focused comment (ADR 0026 §1, BDR 0017 S1): mirrors
    /// `FocusNextComment`'s guard, clamping at the first comment.
    FocusPrevComment,
    /// The one-shot `myself` fetch landed (ADR 0026 §2, BDR 0017 S2): stores
    /// the authenticated user's account id `is_own_comment` compares against.
    MyselfLoaded(String),
    /// `e` on the Detail screen (ADR 0026 §3, BDR 0017 S3, S6): a focused
    /// comment that `is_own_comment` AND has an id opens the compose in edit
    /// mode, pre-filled from the comment body; a focused comment that is not
    /// own sets the localized "not your comment" status with no compose; no
    /// focused comment (or an own comment with no id) is a no-op.
    EditFocusedComment,
}

#[derive(Debug, PartialEq)]
pub enum Cmd {
    Quit,
    LoadDetail(String),
    LoadList(String),
    LoadMore(String, String),
    OpenUrl(String),
    CopyToClipboard(String),
    /// The one-shot entry revalidation fetch (BDR 0008 S1), dispatched by
    /// `entry_cmds` immediately after a warm `Model` is constructed.
    RevalidateList,
    /// The Projects screen's fetch (ADR 0021 §2, BDR 0013 S1): `list_projects`
    /// on 'p', dispatched at most once per Projects screen entry.
    LoadProjects,
    /// The comment compose's submit effect (ADR 0024 §4): posts `body` as a
    /// new comment on `key` via the C1 `add_comment` seam.
    SubmitComment {
        key: String,
        body: String,
    },
    /// The post-mutation server-truth refresh (ADR 0024 §5, BDR 0015 S2):
    /// re-fetches `key`'s detail bypassing the cache-read gate `Cmd::LoadDetail`
    /// uses, so the thread reflects the just-posted comment — reusing
    /// `Cmd::LoadDetail`'s own fetch effect and single-flight discipline (one
    /// spawn, one reply), never a second fetch mechanism.
    RefreshDetail(String),
    /// The one-shot authenticated-identity fetch (ADR 0026 §2, BDR 0017 S2),
    /// dispatched by `entry_cmds` once at browse startup: replies
    /// `Msg::MyselfLoaded` on success; sends nothing on failure, so
    /// `current_account_id` safely stays `None`.
    LoadMyself,
    /// The comment compose's edit-submit effect (ADR 0026 §3, BDR 0017 S4):
    /// PUTs `body` onto `comment_id` on `key` via the `update_comment` seam.
    /// The shell replies the SAME `Msg::CommentMutationOk`/`Err` `SubmitComment`
    /// uses — no new mutation-result arm exists for edit.
    EditComment {
        key: String,
        comment_id: String,
        body: String,
    },
}

/// The `Cmd`s to dispatch right after constructing a fresh `Model` (BDR 0008
/// S1 seam; `Cmd::LoadMyself` added by ADR 0026 §2): every entry dispatches
/// exactly one `Cmd::LoadMyself`; a warm entry (`revalidating: true`) also
/// kicks off exactly one background revalidation.
pub fn entry_cmds(model: &Model) -> Vec<Cmd> {
    let mut cmds = vec![Cmd::LoadMyself];
    if model.revalidating {
        cmds.push(Cmd::RevalidateList);
    }
    cmds
}

/// Pure state transition — no I/O, no terminal, no clock.
pub fn update(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    let model = clear_status_on_key_event(model, &msg);
    if model.compose.is_some() && leaks_through_open_compose(&msg) {
        return (model, vec![]);
    }
    match msg {
        Msg::Up => update_up(model),
        Msg::Down => update_down(model),
        Msg::Quit => (model, vec![Cmd::Quit]),
        Msg::Select => update_select(model),
        Msg::Back => update_back(model),
        Msg::DetailLoaded(issue) => update_detail_loaded(model, issue),
        Msg::OpenSearch => update_open_search(model),
        Msg::SearchInput(c) => update_search_input(model, c),
        Msg::SearchBackspace => update_search_backspace(model),
        Msg::SubmitSearch => update_submit_search(model),
        Msg::CancelSearch => update_cancel_search(model),
        Msg::ListLoaded(rows, token) => update_list_loaded(model, rows, token),
        Msg::MoreLoaded(rows, token) => update_more_loaded(model, rows, token),
        Msg::LoadMore => update_load_more(model),
        Msg::LoadFailed(msg) => update_load_failed(model, msg),
        Msg::OpenLink => update_open_link(model),
        Msg::CopyKey => update_copy_key(model),
        Msg::FocusNextLink => update_focus_next_link(model),
        Msg::RevalidationLoaded(rows, token) => update_revalidation_loaded(model, rows, token),
        Msg::RevalidationFailed(msg) => update_revalidation_failed(model, msg),
        Msg::CardClicked(index) => update_card_clicked(model, index),
        Msg::LinkClicked(href) => update_link_clicked(model, href),
        Msg::SelStart(pos) => update_sel_start(model, pos),
        Msg::SelDrag(pos) => update_sel_drag(model, pos),
        Msg::SelEnd(text) => update_sel_end(model, text),
        Msg::OpenProjects => update_open_projects(model),
        Msg::ProjectsLoaded(rows) => update_projects_loaded(model, rows),
        Msg::ProjectsFailed(msg) => update_projects_failed(model, msg),
        Msg::ProjectClicked(index) => update_project_clicked(model, index),
        Msg::OpenCompose => update_open_compose(model),
        Msg::ComposeInput(c) => update_compose_input(model, c),
        Msg::ComposeNewline => update_compose_newline(model),
        Msg::ComposeBackspace => update_compose_backspace(model),
        Msg::SubmitCompose => update_submit_compose(model),
        Msg::CancelCompose => update_cancel_compose(model),
        Msg::CommentMutationOk => update_comment_mutation_ok(model),
        Msg::CommentMutationErr(reason) => update_comment_mutation_err(model, reason),
        Msg::FocusNextComment => update_focus_next_comment(model),
        Msg::FocusPrevComment => update_focus_prev_comment(model),
        Msg::MyselfLoaded(account_id) => update_myself_loaded(model, account_id),
        Msg::EditFocusedComment => update_edit_focused_comment(model),
    }
}

/// Any key-driven `Msg` dismisses the prior transient status (BDR 0007 S8)
/// before it is processed; replies from spawned `Cmd`s (`DetailLoaded`,
/// `ListLoaded`, `MoreLoaded`, `LoadFailed`) leave a standing status alone —
/// only `LoadFailed` itself sets a new one.
fn clear_status_on_key_event(model: Model, msg: &Msg) -> Model {
    if is_reply_msg(msg) {
        model
    } else {
        Model {
            status: None,
            ..model
        }
    }
}

fn is_reply_msg(msg: &Msg) -> bool {
    matches!(
        msg,
        Msg::DetailLoaded(_)
            | Msg::ListLoaded(_, _)
            | Msg::MoreLoaded(_, _)
            | Msg::LoadFailed(_)
            | Msg::RevalidationLoaded(_, _)
            | Msg::RevalidationFailed(_)
            | Msg::ProjectsLoaded(_)
            | Msg::ProjectsFailed(_)
            | Msg::CommentMutationOk
            | Msg::CommentMutationErr(_)
            | Msg::MyselfLoaded(_)
    )
}

/// While a compose is open, only the compose's own `Msg`s and background
/// replies may reach `update`'s reducers (ADR 0024 §3, BDR 0015 S6): every
/// other key/mouse-resolved `Msg` — list/detail nav, search, links,
/// selection, Projects — is inert, so nothing behind the dimmed backdrop
/// changes and `q` cannot quit while composing.
fn leaks_through_open_compose(msg: &Msg) -> bool {
    let compose_owned = matches!(
        msg,
        Msg::OpenCompose
            | Msg::ComposeInput(_)
            | Msg::ComposeNewline
            | Msg::ComposeBackspace
            | Msg::SubmitCompose
            | Msg::CancelCompose
    );
    !compose_owned && !is_reply_msg(msg)
}

fn update_down(model: Model) -> (Model, Vec<Cmd>) {
    match model.screen {
        Screen::List => {
            let last = model.rows.len().saturating_sub(1);
            let selected = (model.selected + 1).min(last);
            (Model { selected, ..model }, vec![])
        }
        Screen::Projects => {
            let last = model.projects.len().saturating_sub(1);
            let projects_selected = (model.projects_selected + 1).min(last);
            (
                Model {
                    projects_selected,
                    ..model
                },
                vec![],
            )
        }
        Screen::Detail => {
            let detail_scroll = model.detail_scroll.saturating_add(1);
            (
                Model {
                    detail_scroll,
                    ..model
                },
                vec![],
            )
        }
    }
}

fn update_up(model: Model) -> (Model, Vec<Cmd>) {
    match model.screen {
        Screen::List => {
            let selected = model.selected.saturating_sub(1);
            (Model { selected, ..model }, vec![])
        }
        Screen::Projects => {
            let projects_selected = model.projects_selected.saturating_sub(1);
            (
                Model {
                    projects_selected,
                    ..model
                },
                vec![],
            )
        }
        Screen::Detail => {
            let detail_scroll = model.detail_scroll.saturating_sub(1);
            (
                Model {
                    detail_scroll,
                    ..model
                },
                vec![],
            )
        }
    }
}

/// `Select` activates the current screen's focus: on `List` it opens the
/// selected row's detail (unchanged); on `Detail` it opens the focused inline
/// link, if any.
fn update_select(model: Model) -> (Model, Vec<Cmd>) {
    match model.screen {
        Screen::List => update_select_list(model),
        Screen::Projects => update_select_projects(model),
        Screen::Detail => update_select_focused_link(model),
    }
}

fn update_select_list(model: Model) -> (Model, Vec<Cmd>) {
    if model.rows.is_empty() {
        return (model, vec![]);
    }
    let key = model.rows[model.selected].key.clone();
    let next = Model {
        screen: Screen::Detail,
        detail: None,
        detail_scroll: 0,
        ..model
    };
    (next, vec![Cmd::LoadDetail(key)])
}

/// A left click on a visible list card (ADR 0017 §4, BDR 0009 S3): in range on
/// the list screen it sets `selected` then delegates to `update_select_list`
/// — the exact same open-detail contract as `Select`/`Enter`. Out-of-range,
/// an empty list, or a Detail-screen click are pure no-ops (BDR 0009 S4); the
/// resolver already filters most of these, so this guard is defense in depth.
fn update_card_clicked(model: Model, index: usize) -> (Model, Vec<Cmd>) {
    if model.screen != Screen::List || index >= model.rows.len() {
        return (model, vec![]);
    }
    update_select_list(Model {
        selected: index,
        ..model
    })
}

/// Enter on the Projects screen drills into the selected project (ADR 0021
/// §4, BDR 0013 S3): a no-op with no projects loaded.
fn update_select_projects(model: Model) -> (Model, Vec<Cmd>) {
    if model.projects.is_empty() {
        return (model, vec![]);
    }
    let project = model.projects[model.projects_selected].clone();
    drill_into_project(model, project)
}

/// A left click on a visible Projects row (ADR 0021, BDR 0013 S2-S3): in
/// range it sets `projects_selected` then delegates to
/// `update_select_projects` — the exact same drill-in contract as
/// `Select`/`Enter`. Out-of-range, an empty list, or a non-Projects-screen
/// click are pure no-ops, mirroring `update_card_clicked`'s defense in depth.
fn update_project_clicked(model: Model, index: usize) -> (Model, Vec<Cmd>) {
    if model.screen != Screen::Projects || index >= model.projects.len() {
        return (model, vec![]);
    }
    update_select_projects(Model {
        projects_selected: index,
        ..model
    })
}

/// Drills into `project`'s issues (ADR 0021 §4): sets the origin/JQL, clears
/// the list state (rows, selection, paging cursor, search), returns to
/// `Screen::List`, and emits the same `Cmd::LoadList` shape
/// `update_submit_search` uses — so pagination, search, and detail all apply
/// unchanged to the project's issues (BDR 0013 S3).
fn drill_into_project(model: Model, project: ProjectRow) -> (Model, Vec<Cmd>) {
    let jql = format!("project = {} ORDER BY updated DESC", project.key);
    let next = Model {
        screen: Screen::List,
        list_origin: ListOrigin::Project(project.key),
        jql: jql.clone(),
        rows: vec![],
        selected: 0,
        next_page_token: None,
        search: None,
        error: None,
        ..model
    };
    (next, vec![Cmd::LoadList(jql)])
}

fn update_select_focused_link(model: Model) -> (Model, Vec<Cmd>) {
    match model.detail_focused_link {
        Some(i) => {
            let url = model.detail_links[i].clone();
            (model, vec![Cmd::OpenUrl(url)])
        }
        None => (model, vec![]),
    }
}

/// A Ctrl/Super-click resolved to a link's href (ADR 0018 §4, BDR 0010 S5):
/// emits `Cmd::OpenUrl` with no state change on the Detail screen, mirroring
/// `update_select_focused_link`'s Cmd contract; a no-op on any other screen
/// (`resolve_click` only ever resolves this on Detail, but this stays a pure
/// guard rather than trusting the caller).
fn update_link_clicked(model: Model, href: String) -> (Model, Vec<Cmd>) {
    if model.screen != Screen::Detail {
        return (model, vec![]);
    }
    (model, vec![Cmd::OpenUrl(href)])
}

/// Back pops the axis by `(screen, list_origin)` (ADR 0021 §5, BDR 0013 S4;
/// `Search` added by ADR 0025/BDR 0016 §S2): Detail returns to the list it
/// came from, or exits the TUI for a seeded top-level detail (ADR 0025 §3,
/// BDR 0016 S7 — see `back_from_detail`); a `Project`-origin list returns to
/// Projects (rows retained, no refetch); a `Mine`- or `Search`-origin list
/// is a no-op (today's behavior — there is no screen behind a top-level
/// list); Projects with a `Project` origin restores the mine list
/// (reloaded); Projects with the `Mine`/`Search` origin goes straight back
/// to the list with its rows intact (no refetch — nothing was ever
/// replaced).
fn update_back(model: Model) -> (Model, Vec<Cmd>) {
    match model.screen {
        Screen::Detail => back_from_detail(model),
        Screen::List => back_from_list(model),
        Screen::Projects => back_from_projects(model),
    }
}

/// A seeded top-level detail (ADR 0025 §3, `TuiSeed::Detail`) is the only
/// path that leaves `jql` empty (`seeded_model` sets `jql: String::new()`
/// for it); every drilled-in detail carries a non-empty Mine/Search/Project
/// jql from the list it came from. Unlike `rows.is_empty()`, this signal is
/// revalidation-immune: `update_revalidation_loaded` can legitimately swap a
/// drilled-in detail's underlying list down to zero rows (an empty
/// revalidation result) while the screen is still `Detail`, but it never
/// touches `jql` — so an empty-rows check would misfire `Cmd::Quit` on a
/// drilled-in detail that should instead return to its list.
fn back_from_detail(model: Model) -> (Model, Vec<Cmd>) {
    if model.jql.is_empty() {
        return (model, vec![Cmd::Quit]);
    }
    let next = Model {
        screen: Screen::List,
        detail: None,
        detail_links: vec![],
        detail_focused_link: None,
        detail_focused_comment: None,
        selection: None,
        ..model
    };
    (next, vec![])
}

fn back_from_list(model: Model) -> (Model, Vec<Cmd>) {
    match model.list_origin {
        ListOrigin::Project(_) => {
            let next = Model {
                screen: Screen::Projects,
                ..model
            };
            (next, vec![])
        }
        ListOrigin::Mine | ListOrigin::Search => (model, vec![]),
    }
}

fn back_from_projects(model: Model) -> (Model, Vec<Cmd>) {
    match model.list_origin {
        ListOrigin::Project(_) => restore_mine_list(model),
        ListOrigin::Mine | ListOrigin::Search => {
            let next = Model {
                screen: Screen::List,
                ..model
            };
            (next, vec![])
        }
    }
}

/// Restores the mine list (ADR 0021 §5): origin back to `Mine`, the shared
/// `MINE_JQL` constant (the single source, also used at browse entry),
/// cleared list state, and the same `Cmd::LoadList` shape every other
/// list-replacing transition emits — a reload is required since the
/// project's issues replaced the rows.
fn restore_mine_list(model: Model) -> (Model, Vec<Cmd>) {
    let next = Model {
        screen: Screen::List,
        list_origin: ListOrigin::Mine,
        jql: MINE_JQL.to_owned(),
        rows: vec![],
        selected: 0,
        next_page_token: None,
        search: None,
        error: None,
        ..model
    };
    (next, vec![Cmd::LoadList(MINE_JQL.to_owned())])
}

/// Unmodified left DOWN anchors a fresh selection on the Detail screen,
/// replacing any previous one (BDR 0011 S1); a no-op on other screens.
fn update_sel_start(model: Model, pos: (usize, usize)) -> (Model, Vec<Cmd>) {
    if model.screen != Screen::Detail {
        return (model, vec![]);
    }
    let next = Model {
        selection: Some(Selection {
            anchor: pos,
            cursor: pos,
            dragged: false,
        }),
        ..model
    };
    (next, vec![])
}

/// A drag extends the active selection's cursor and marks it dragged (BDR
/// 0011 S1); a no-op with no active selection or off the Detail screen.
fn update_sel_drag(model: Model, pos: (usize, usize)) -> (Model, Vec<Cmd>) {
    if model.screen != Screen::Detail {
        return (model, vec![]);
    }
    let Some(selection) = model.selection.clone() else {
        return (model, vec![]);
    };
    let next = Model {
        selection: Some(Selection {
            cursor: pos,
            dragged: true,
            ..selection
        }),
        ..model
    };
    (next, vec![])
}

/// Release after a drag copies the selected text and shows the existing
/// "Copied" status, keeping the highlight (BDR 0011 S2); release without a
/// drag (`None`) clears the selection with no `Cmd` and no navigation (S3); a
/// no-op off the Detail screen.
fn update_sel_end(model: Model, text: Option<String>) -> (Model, Vec<Cmd>) {
    if model.screen != Screen::Detail {
        return (model, vec![]);
    }
    match text {
        Some(text) => {
            let next = Model {
                status: Some(StatusMsg {
                    text: t("Copied ✓"),
                    kind: StatusKind::Info,
                }),
                ..model
            };
            (next, vec![Cmd::CopyToClipboard(text)])
        }
        None => {
            let next = Model {
                selection: None,
                ..model
            };
            (next, vec![])
        }
    }
}

fn update_detail_loaded(model: Model, issue: Box<Issue>) -> (Model, Vec<Cmd>) {
    let detail_links = description_link_hrefs(&issue);
    let detail_focused_link = (!detail_links.is_empty()).then_some(0);
    let next = Model {
        detail: Some(*issue),
        detail_scroll: 0,
        detail_links,
        detail_focused_link,
        detail_focused_comment: None,
        ..model
    };
    (next, vec![])
}

/// Collects the description's inline `link`-mark hrefs, in document order,
/// from the same `adf_to_rich` model the view renders (so link ordering
/// matches between `detail_links` and the displayed spans).
fn description_link_hrefs(issue: &Issue) -> Vec<String> {
    issue
        .description
        .as_deref()
        .map(adf_to_rich)
        .unwrap_or_default()
        .iter()
        .flat_map(|line| line.iter())
        .filter_map(|span| span.style.link.clone())
        .collect()
}

fn update_open_search(model: Model) -> (Model, Vec<Cmd>) {
    if model.screen != Screen::List {
        return (model, vec![]);
    }
    let next = Model {
        search: Some(String::new()),
        error: None,
        ..model
    };
    (next, vec![])
}

/// `p` opens the Projects screen from the issue list (ADR 0021 §1, BDR 0013
/// S1): only from `Screen::List` with search inactive (the key mapper
/// already never emits this while search is active; the guard here is
/// defense in depth, mirroring `update_open_search`). Existing `projects`
/// rows are kept (not cleared) while the fresh fetch is in flight, and a
/// loading status is set on the existing status seam.
fn update_open_projects(model: Model) -> (Model, Vec<Cmd>) {
    if model.screen != Screen::List || model.search.is_some() {
        return (model, vec![]);
    }
    let next = Model {
        screen: Screen::Projects,
        projects_selected: 0,
        status: Some(StatusMsg {
            text: t(LOADING_STATUS_TEXT),
            kind: StatusKind::Info,
        }),
        ..model
    };
    (next, vec![Cmd::LoadProjects])
}

/// The projects fetch completed (BDR 0013 S1): sets the fetched rows,
/// clamps `projects_selected` into range, and clears the loading status. A
/// late result while off the Projects screen still updates the data
/// (harmless) but never changes `screen`.
fn update_projects_loaded(model: Model, rows: Vec<ProjectRow>) -> (Model, Vec<Cmd>) {
    let projects_selected = model.projects_selected.min(rows.len().saturating_sub(1));
    let next = Model {
        projects: rows,
        projects_selected,
        status: None,
        ..model
    };
    (next, vec![])
}

/// A projects fetch failure (BDR 0013 S5): an Error status on the existing
/// status seam, staying on the Projects screen (never changes `screen`).
fn update_projects_failed(model: Model, msg: String) -> (Model, Vec<Cmd>) {
    let next = Model {
        status: Some(StatusMsg {
            text: msg,
            kind: StatusKind::Error,
        }),
        ..model
    };
    (next, vec![])
}

fn update_search_input(model: Model, c: char) -> (Model, Vec<Cmd>) {
    let search = match model.search {
        Some(mut q) => {
            q.push(c);
            Some(q)
        }
        None => None,
    };
    (Model { search, ..model }, vec![])
}

fn update_search_backspace(model: Model) -> (Model, Vec<Cmd>) {
    let search = match model.search {
        Some(mut q) => {
            q.pop();
            Some(q)
        }
        None => None,
    };
    (Model { search, ..model }, vec![])
}

/// Submitting a search also clears `revalidating` (BDR 0008 S4), so an
/// eventual late `RevalidationLoaded`/`RevalidationFailed` from the entry
/// revalidation is ignored — the fresher search result wins.
fn update_submit_search(model: Model) -> (Model, Vec<Cmd>) {
    match &model.search {
        Some(q) if !q.is_empty() => {
            let jql = q.clone();
            let next = Model {
                jql: jql.clone(),
                revalidating: false,
                ..model
            };
            (next, vec![Cmd::LoadList(jql)])
        }
        _ => (model, vec![]),
    }
}

fn update_cancel_search(model: Model) -> (Model, Vec<Cmd>) {
    let next = Model {
        search: None,
        ..model
    };
    (next, vec![])
}

/// A fresh list/search result replaces the rows and resets the paging cursor
/// from the new result's token.
fn update_list_loaded(
    model: Model,
    rows: Vec<IssueRow>,
    next_page_token: Option<String>,
) -> (Model, Vec<Cmd>) {
    let next = Model {
        rows,
        selected: 0,
        search: None,
        error: None,
        next_page_token,
        ..model
    };
    (next, vec![])
}

/// A load-more page appends to the existing rows, preserving selection, and
/// advances the paging cursor to the new token.
fn update_more_loaded(
    model: Model,
    rows: Vec<IssueRow>,
    next_page_token: Option<String>,
) -> (Model, Vec<Cmd>) {
    let mut all_rows = model.rows;
    all_rows.extend(rows);
    let next = Model {
        rows: all_rows,
        next_page_token,
        ..model
    };
    (next, vec![])
}

/// Emits `Cmd::LoadMore` only on the list screen with a pending paging
/// cursor; a no-op otherwise (last page already loaded, or on Detail), and
/// while a revalidation is in flight — dropped, not queued (BDR 0008 S6).
fn update_load_more(model: Model) -> (Model, Vec<Cmd>) {
    if model.revalidating {
        return (model, vec![]);
    }
    if model.screen == Screen::List {
        if let Some(token) = model.next_page_token.clone() {
            let jql = model.jql.clone();
            return (model, vec![Cmd::LoadMore(jql, token)]);
        }
    }
    (model, vec![])
}

/// A fetch error sets both the persistent search-JQL banner (BDR 0006 S5,
/// unchanged) and the transient status row (BDR 0007 S8) in the error style.
fn update_load_failed(model: Model, msg: String) -> (Model, Vec<Cmd>) {
    let next = Model {
        status: Some(StatusMsg {
            text: msg.clone(),
            kind: StatusKind::Error,
        }),
        error: Some(msg),
        search: None,
        ..model
    };
    (next, vec![])
}

fn update_open_link(model: Model) -> (Model, Vec<Cmd>) {
    if model.rows.is_empty() {
        return (model, vec![]);
    }
    let url = crate::render::issue_browse_url(&model.base_url, &model.rows[model.selected].key);
    (model, vec![Cmd::OpenUrl(url)])
}

/// Copies the selected issue key and shows an Info confirmation on the
/// status row (BDR 0007 S8); a no-op with no rows and no status change.
fn update_copy_key(model: Model) -> (Model, Vec<Cmd>) {
    if model.rows.is_empty() {
        return (model, vec![]);
    }
    let key = model.rows[model.selected].key.clone();
    let next = Model {
        status: Some(StatusMsg {
            text: t("Copied ✓"),
            kind: StatusKind::Info,
        }),
        ..model
    };
    (next, vec![Cmd::CopyToClipboard(key)])
}

/// Advances the focused inline link, wrapping, when on the Detail screen with
/// links present; a no-op (empty cmds) on the List screen or with no links.
fn update_focus_next_link(model: Model) -> (Model, Vec<Cmd>) {
    if model.screen != Screen::Detail || model.detail_links.is_empty() {
        return (model, vec![]);
    }
    let len = model.detail_links.len();
    let next_index = model.detail_focused_link.map_or(0, |i| (i + 1) % len);
    let next = Model {
        detail_focused_link: Some(next_index),
        ..model
    };
    (next, vec![])
}

/// A background revalidation swaps in the fresh rows, clamps selection (not
/// reset), and restores the paging cursor (BDR 0008 S2) — but only while
/// `revalidating` is still true; a stale result arriving after a newer
/// action (which clears the flag) is a pure no-op (S4).
fn update_revalidation_loaded(
    model: Model,
    rows: Vec<IssueRow>,
    next_page_token: Option<String>,
) -> (Model, Vec<Cmd>) {
    if !model.revalidating {
        return (model, vec![]);
    }
    let selected = model.selected.min(rows.len().saturating_sub(1));
    let next = Model {
        rows,
        selected,
        next_page_token,
        revalidating: false,
        ..model
    };
    (next, vec![])
}

/// A failed revalidation keeps the painted rows and surfaces the message on
/// the status row in the Error style (BDR 0008 S5) — but only while
/// `revalidating` is still true; a no-op otherwise (S4).
fn update_revalidation_failed(model: Model, msg: String) -> (Model, Vec<Cmd>) {
    if !model.revalidating {
        return (model, vec![]);
    }
    let next = Model {
        revalidating: false,
        status: Some(StatusMsg {
            text: msg,
            kind: StatusKind::Error,
        }),
        ..model
    };
    (next, vec![])
}

/// `c` opens the comment compose only on `Screen::Detail` with a loaded
/// issue (BDR 0015 S1, S8): a no-op on List/Projects (so `c` keeps its
/// absent meaning there) and on a Detail screen still loading, which has no
/// issue key to attach the comment to.
fn update_open_compose(model: Model) -> (Model, Vec<Cmd>) {
    if model.screen != Screen::Detail || model.detail.is_none() {
        return (model, vec![]);
    }
    let next = Model {
        compose: Some(Compose {
            buffer: String::new(),
            status: ComposeStatus::Idle,
            target: ComposeTarget::New,
        }),
        ..model
    };
    (next, vec![])
}

fn update_compose_input(model: Model, c: char) -> (Model, Vec<Cmd>) {
    update_compose_buffer(model, |buffer| buffer.push(c))
}

/// Enter inserts a newline into the buffer — it never submits (BDR 0015 S1);
/// submitting is `Ctrl+S`'s job alone (`update_submit_compose`).
fn update_compose_newline(model: Model) -> (Model, Vec<Cmd>) {
    update_compose_buffer(model, |buffer| buffer.push('\n'))
}

fn update_compose_backspace(model: Model) -> (Model, Vec<Cmd>) {
    update_compose_buffer(model, |buffer| {
        buffer.pop();
    })
}

/// Applies `edit` to the open compose's buffer; a no-op with no compose
/// open. The single seam every buffer-mutating `Msg` routes through, so
/// `ComposeInput`/`ComposeNewline`/`ComposeBackspace` can never drift on how
/// the buffer is read or written back.
fn update_compose_buffer(model: Model, edit: impl FnOnce(&mut String)) -> (Model, Vec<Cmd>) {
    let Some(mut compose) = model.compose.clone() else {
        return (model, vec![]);
    };
    edit(&mut compose.buffer);
    let next = Model {
        compose: Some(compose),
        ..model
    };
    (next, vec![])
}

/// Ctrl+S submits the open compose (BDR 0015 S2, S7; BDR 0017 S4): a
/// non-empty (trimmed) buffer emits exactly one submit `Cmd` for the Detail
/// screen's loaded issue and sets `Submitting` — `SubmitComment` for a `New`
/// target (unchanged), `EditComment` for an `Edit` target; an empty/whitespace
/// buffer is a no-op and the modal stays open unchanged. A no-op with no
/// compose open or no loaded issue — defense in depth, since the compose only
/// ever opens with one (mirrors `update_card_clicked`'s guard style).
fn update_submit_compose(model: Model) -> (Model, Vec<Cmd>) {
    let Some(compose) = model.compose.clone() else {
        return (model, vec![]);
    };
    if compose.buffer.trim().is_empty() {
        return (model, vec![]);
    }
    let Some(issue) = model.detail.as_ref() else {
        return (model, vec![]);
    };
    let key = issue.key.clone();
    let body = compose.buffer.clone();
    let cmd = submit_compose_cmd(&compose.target, key, body);
    let next = Model {
        compose: Some(Compose {
            status: ComposeStatus::Submitting,
            ..compose
        }),
        ..model
    };
    (next, vec![cmd])
}

/// The submit `Cmd` a compose's `target` emits (BDR 0017 S4): `New` posts a
/// brand-new comment, `Edit` PUTs onto the identified comment. The single
/// seam `update_submit_compose` routes through, so the two write paths can
/// never drift on how `key`/`body` are carried.
fn submit_compose_cmd(target: &ComposeTarget, key: String, body: String) -> Cmd {
    match target {
        ComposeTarget::New => Cmd::SubmitComment { key, body },
        ComposeTarget::Edit { comment_id } => Cmd::EditComment {
            key,
            comment_id: comment_id.clone(),
            body,
        },
    }
}

/// Esc discards the open compose with no write and no refresh, leaving the
/// detail state (scroll/selection/links) untouched (BDR 0015 S3).
fn update_cancel_compose(model: Model) -> (Model, Vec<Cmd>) {
    let next = Model {
        compose: None,
        ..model
    };
    (next, vec![])
}

/// The comment write succeeded (ADR 0024 §5, BDR 0015 S2): the compose
/// closes and exactly one cache-busting refresh is emitted for the open
/// issue — never a locally-fabricated comment (the server owns
/// id/author/timestamp). No refresh key with no loaded issue never happens
/// in practice (the compose only opens with one), guarded defensively.
fn update_comment_mutation_ok(model: Model) -> (Model, Vec<Cmd>) {
    let cmds = match model.detail.as_ref() {
        Some(issue) => vec![Cmd::RefreshDetail(issue.key.clone())],
        None => vec![],
    };
    let next = Model {
        compose: None,
        ..model
    };
    (next, cmds)
}

/// The comment write failed (ADR 0024 §5, BDR 0015 S4): the buffer is
/// preserved, the compose shows the localized `reason` (a 401 reuses the E2
/// re-auth message), and no refresh Cmd is emitted. A no-op with no compose
/// open.
fn update_comment_mutation_err(model: Model, reason: String) -> (Model, Vec<Cmd>) {
    let Some(compose) = model.compose.clone() else {
        return (model, vec![]);
    };
    let next = Model {
        compose: Some(Compose {
            status: ComposeStatus::Error(reason),
            ..compose
        }),
        ..model
    };
    (next, vec![])
}

/// `]` advances the focused comment (ADR 0026 §1, BDR 0017 S1): an unset
/// focus starts at the first comment, clamping at the last; a no-op with no
/// comments to focus (`focused_comment_len`'s guard).
fn update_focus_next_comment(model: Model) -> (Model, Vec<Cmd>) {
    let Some(len) = focused_comment_len(&model) else {
        return (model, vec![]);
    };
    let next_index = model
        .detail_focused_comment
        .map_or(0, |i| (i + 1).min(len - 1));
    (
        Model {
            detail_focused_comment: Some(next_index),
            ..model
        },
        vec![],
    )
}

/// `[` retreats the focused comment (ADR 0026 §1, BDR 0017 S1): an unset
/// focus starts at the last comment, clamping at the first; a no-op with no
/// comments to focus (`focused_comment_len`'s guard).
fn update_focus_prev_comment(model: Model) -> (Model, Vec<Cmd>) {
    let Some(len) = focused_comment_len(&model) else {
        return (model, vec![]);
    };
    let next_index = model
        .detail_focused_comment
        .map_or(len - 1, |i| i.saturating_sub(1));
    (
        Model {
            detail_focused_comment: Some(next_index),
            ..model
        },
        vec![],
    )
}

/// The focused-comment axis's length guard (ADR 0026 §1, BDR 0017 S1): `Some`
/// only on `Screen::Detail` with a loaded issue whose comment thread is
/// non-empty — the shared precondition `update_focus_next_comment` and
/// `update_focus_prev_comment` both require before moving the index.
fn focused_comment_len(model: &Model) -> Option<usize> {
    if model.screen != Screen::Detail {
        return None;
    }
    let issue = model.detail.as_ref()?;
    (!issue.comments.is_empty()).then_some(issue.comments.len())
}

/// The one-shot `myself` fetch landed (ADR 0026 §2, BDR 0017 S2): stores the
/// authenticated user's account id `is_own_comment` compares against. Emits
/// no `Cmd`.
fn update_myself_loaded(model: Model, account_id: String) -> (Model, Vec<Cmd>) {
    let next = Model {
        current_account_id: Some(account_id),
        ..model
    };
    (next, vec![])
}

/// `e` opens the compose in edit mode for the focused OWN comment (ADR 0026
/// §3, BDR 0017 S3, S6): no focused comment is a no-op; a focused comment
/// that is not `is_own_comment` sets the localized "not your comment" status
/// and opens no compose; an own comment with no `id` (never happens for a
/// server-returned comment, guarded defensively) is also a no-op; otherwise
/// the compose opens with `target: Edit{comment_id}`, `buffer` pre-filled via
/// `adf_to_plain_text`, and `status: Idle`.
fn update_edit_focused_comment(model: Model) -> (Model, Vec<Cmd>) {
    let Some(comment) = focused_comment(&model) else {
        return (model, vec![]);
    };
    if !model.is_own_comment(comment) {
        return (set_not_your_comment_status(model), vec![]);
    }
    let Some(comment_id) = comment.id.clone() else {
        return (model, vec![]);
    };
    let buffer = adf_to_plain_text(&comment.body);
    let next = Model {
        compose: Some(Compose {
            buffer,
            status: ComposeStatus::Idle,
            target: ComposeTarget::Edit { comment_id },
        }),
        ..model
    };
    (next, vec![])
}

/// The currently focused comment on the Detail screen, or `None` when off
/// Detail, with no loaded issue, or with no focus set — the shared
/// precondition `update_edit_focused_comment` (and a future delete/reply)
/// reads before acting on "the focused comment".
fn focused_comment(model: &Model) -> Option<&IssueComment> {
    if model.screen != Screen::Detail {
        return None;
    }
    let issue = model.detail.as_ref()?;
    let index = model.detail_focused_comment?;
    issue.comments.get(index)
}

/// Sets the localized "not your comment" transient status (ADR 0026 §3, BDR
/// 0017 S6), reusing the existing status row rather than a bespoke seam.
fn set_not_your_comment_status(model: Model) -> Model {
    Model {
        status: Some(StatusMsg {
            text: t("Not your comment"),
            kind: StatusKind::Info,
        }),
        ..model
    }
}

#[cfg(test)]
#[path = "../../tests/unit/tui/model.rs"]
mod tests;
