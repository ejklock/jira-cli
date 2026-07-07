use crate::i18n::t;
use crate::models::{Issue, IssueRow};
use crate::render::adf_to_rich;

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    List,
    Detail,
}

/// The footer's mode-aware hint bucket (ADR 0014 §5, BDR 0007 S7): derived
/// purely from the current screen + search-input-active + focused-link state,
/// so `footer_hint` never branches on `Model` fields directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FooterMode {
    List,
    ListSearch,
    Detail,
    DetailLink,
}

/// Derives the active [`FooterMode`] from the model's screen, search-input,
/// and focused-link state (BDR 0007 S7). Pure — no rendering, no I/O.
pub fn footer_mode(model: &Model) -> FooterMode {
    match model.screen {
        Screen::List if model.search.is_some() => FooterMode::ListSearch,
        Screen::List => FooterMode::List,
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
}

/// The `Cmd`s to dispatch right after constructing a fresh `Model` (BDR 0008
/// S1 seam): a warm entry (`revalidating: true`) kicks off exactly one
/// background revalidation; a cold entry emits nothing.
pub fn entry_cmds(model: &Model) -> Vec<Cmd> {
    if model.revalidating {
        vec![Cmd::RevalidateList]
    } else {
        vec![]
    }
}

/// Pure state transition — no I/O, no terminal, no clock.
pub fn update(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
    let model = clear_status_on_key_event(model, &msg);
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
    )
}

fn update_down(model: Model) -> (Model, Vec<Cmd>) {
    match model.screen {
        Screen::List => {
            let last = model.rows.len().saturating_sub(1);
            let selected = (model.selected + 1).min(last);
            (Model { selected, ..model }, vec![])
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

fn update_back(model: Model) -> (Model, Vec<Cmd>) {
    let next = Model {
        screen: Screen::List,
        detail: None,
        detail_links: vec![],
        detail_focused_link: None,
        selection: None,
        ..model
    };
    (next, vec![])
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
