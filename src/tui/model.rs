use crate::models::{Issue, IssueRow};

#[derive(Debug, Clone, PartialEq)]
pub enum Screen {
    List,
    Detail,
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
    ListLoaded(Vec<IssueRow>),
    LoadFailed(String),
    OpenLink,
    CopyKey,
}

#[derive(Debug, PartialEq)]
pub enum Cmd {
    Quit,
    LoadDetail(String),
    LoadList(String),
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
        Msg::ListLoaded(rows) => update_list_loaded(model, rows),
        Msg::LoadFailed(msg) => update_load_failed(model, msg),
        Msg::OpenLink => update_open_link(model),
        Msg::CopyKey => update_copy_key(model),
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

fn update_select(model: Model) -> (Model, Vec<Cmd>) {
    if model.screen != Screen::List || model.rows.is_empty() {
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

fn update_back(model: Model) -> (Model, Vec<Cmd>) {
    let next = Model {
        screen: Screen::List,
        detail: None,
        ..model
    };
    (next, vec![])
}

fn update_detail_loaded(model: Model, issue: Box<Issue>) -> (Model, Vec<Cmd>) {
    let next = Model {
        detail: Some(*issue),
        detail_scroll: 0,
        ..model
    };
    (next, vec![])
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
            (model, vec![Cmd::LoadList(jql)])
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

fn update_list_loaded(model: Model, rows: Vec<IssueRow>) -> (Model, Vec<Cmd>) {
    let next = Model {
        rows,
        selected: 0,
        search: None,
        error: None,
        ..model
    };
    (next, vec![])
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
