use crate::models::{Issue, IssueRow};
use crate::render::adf_to_rich;

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    List,
    Detail,
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
}

#[derive(Debug, PartialEq)]
pub enum Cmd {
    Quit,
    LoadDetail(String),
    LoadList(String),
    LoadMore(String, String),
    OpenUrl(String),
    CopyToClipboard(String),
}

/// Pure state transition — no I/O, no terminal, no clock.
pub fn update(model: Model, msg: Msg) -> (Model, Vec<Cmd>) {
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
    }
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

fn update_select_focused_link(model: Model) -> (Model, Vec<Cmd>) {
    match model.detail_focused_link {
        Some(i) => {
            let url = model.detail_links[i].clone();
            (model, vec![Cmd::OpenUrl(url)])
        }
        None => (model, vec![]),
    }
}

fn update_back(model: Model) -> (Model, Vec<Cmd>) {
    let next = Model {
        screen: Screen::List,
        detail: None,
        detail_links: vec![],
        detail_focused_link: None,
        ..model
    };
    (next, vec![])
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

fn update_submit_search(model: Model) -> (Model, Vec<Cmd>) {
    match &model.search {
        Some(q) if !q.is_empty() => {
            let jql = q.clone();
            let next = Model {
                jql: jql.clone(),
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
/// cursor; a no-op otherwise (last page already loaded, or on Detail).
fn update_load_more(model: Model) -> (Model, Vec<Cmd>) {
    if model.screen == Screen::List {
        if let Some(token) = model.next_page_token.clone() {
            let jql = model.jql.clone();
            return (model, vec![Cmd::LoadMore(jql, token)]);
        }
    }
    (model, vec![])
}

fn update_load_failed(model: Model, msg: String) -> (Model, Vec<Cmd>) {
    let next = Model {
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

fn update_copy_key(model: Model) -> (Model, Vec<Cmd>) {
    if model.rows.is_empty() {
        return (model, vec![]);
    }
    let key = model.rows[model.selected].key.clone();
    (model, vec![Cmd::CopyToClipboard(key)])
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
