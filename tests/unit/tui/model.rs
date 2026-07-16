use super::*;

// ---- Helpers ----

fn make_row(key: &str) -> IssueRow {
    IssueRow {
        key: key.to_owned(),
        issue_type: "Task".to_owned(),
        summary: "Summary".to_owned(),
        status: "Open".to_owned(),
        assignee: None,
        duedate: None,
        project: None,
    }
}

fn make_rows(keys: &[&str]) -> Vec<IssueRow> {
    keys.iter().map(|k| make_row(k)).collect()
}

fn project_row(key: &str, name: &str) -> ProjectRow {
    ProjectRow {
        key: key.to_owned(),
        name: name.to_owned(),
    }
}

fn make_list_model(keys: &[&str], list_origin: ListOrigin, jql: &str) -> Model {
    Model {
        rows: make_rows(keys),
        selected: 0,
        screen: Screen::List,
        detail: None,
        detail_scroll: 0,
        search: None,
        error: None,
        base_url: "https://test.atlassian.net".to_owned(),
        jql: jql.to_owned(),
        next_page_token: None,
        detail_links: vec![],
        detail_focused_link: None,
        identities: vec![],
        status: None,
        revalidating: false,
        selection: None,
        list_origin,
        projects: vec![],
        projects_selected: 0,
        compose: None,
        detail_focused_comment: None,
        current_account_id: None,
    }
}

fn make_detail_model(rows: Vec<IssueRow>, list_origin: ListOrigin, jql: &str) -> Model {
    Model {
        rows,
        selected: 0,
        screen: Screen::Detail,
        detail: Some(crate::test_support::issue("PROJ-1")),
        detail_scroll: 0,
        search: None,
        error: None,
        base_url: "https://test.atlassian.net".to_owned(),
        jql: jql.to_owned(),
        next_page_token: None,
        detail_links: vec![],
        detail_focused_link: None,
        identities: vec![],
        status: None,
        revalidating: false,
        selection: None,
        list_origin,
        projects: vec![],
        projects_selected: 0,
        compose: None,
        detail_focused_comment: None,
        current_account_id: None,
    }
}

fn make_projects_model(projects: Vec<ProjectRow>, list_origin: ListOrigin, jql: &str) -> Model {
    Model {
        rows: vec![],
        selected: 0,
        screen: Screen::Projects,
        detail: None,
        detail_scroll: 0,
        search: None,
        error: None,
        base_url: "https://test.atlassian.net".to_owned(),
        jql: jql.to_owned(),
        next_page_token: None,
        detail_links: vec![],
        detail_focused_link: None,
        identities: vec![],
        status: None,
        revalidating: false,
        selection: None,
        list_origin,
        projects,
        projects_selected: 0,
        compose: None,
        detail_focused_comment: None,
        current_account_id: None,
    }
}

// ---- ac3 (S2): extending ListOrigin with `Search` does not regress the
// existing Mine/Project back-axis behavior, and Search itself behaves like
// Mine — a top-level list with no screen behind it (ADR 0025, BDR 0016 S5) ----

#[test]
fn back_from_list_with_search_origin_is_a_noop() {
    let model = make_list_model(&["NIKE-1", "NIKE-2"], ListOrigin::Search, "project = NIKE");

    let (next, cmds) = update(model, Msg::Back);

    assert_eq!(next.screen, Screen::List);
    assert_eq!(next.list_origin, ListOrigin::Search);
    assert_eq!(next.jql, "project = NIKE");
    assert!(
        cmds.is_empty(),
        "a top-level Search list has no screen behind it, so Back is a no-op"
    );
}

#[test]
fn back_from_projects_with_search_origin_returns_to_list_with_rows_intact_no_cmd() {
    let mut model = make_projects_model(
        vec![project_row("ALPHA", "Alpha Project")],
        ListOrigin::Search,
        "project = NIKE",
    );
    model.rows = make_rows(&["NIKE-1", "NIKE-2"]);

    let (next, cmds) = update(model, Msg::Back);

    assert_eq!(next.screen, Screen::List);
    assert_eq!(next.list_origin, ListOrigin::Search);
    assert_eq!(next.jql, "project = NIKE");
    assert_eq!(
        next.rows.iter().map(|r| r.key.clone()).collect::<Vec<_>>(),
        vec!["NIKE-1".to_owned(), "NIKE-2".to_owned()],
        "the search rows were never replaced, so nothing needs reloading"
    );
    assert!(
        cmds.is_empty(),
        "nothing was replaced, so no Cmd is emitted"
    );
}

#[test]
fn back_from_list_with_mine_origin_still_regresses_to_a_noop() {
    let model = make_list_model(&["MINE-1"], ListOrigin::Mine, MINE_JQL);

    let (next, cmds) = update(model, Msg::Back);

    assert_eq!(next.screen, Screen::List);
    assert_eq!(next.list_origin, ListOrigin::Mine);
    assert!(cmds.is_empty());
}

// ---- ac2 (S3): Back from a seeded top-level detail (empty jql, ADR 0025
// §3 — `seeded_model` sets `jql: String::new()` for `TuiSeed::Detail`) exits
// the TUI; Back from a detail drilled into a list (non-empty jql) still
// returns to that list — no regression ----

#[test]
fn back_from_detail_with_empty_jql_exits_the_tui() {
    let model = make_detail_model(vec![], ListOrigin::Mine, "");

    let (next, cmds) = update(model, Msg::Back);

    assert_eq!(
        next.screen,
        Screen::Detail,
        "quitting emits no screen change"
    );
    assert_eq!(cmds, vec![Cmd::Quit]);
}

// Round-2 ac2 fix regression: a drilled-in Detail carries a non-empty jql
// and can have its underlying list's rows revalidated down to empty while
// the screen is still Detail (`update_revalidation_loaded` has no screen
// guard) — Back must still return to the list, not Quit, because the
// rows.is_empty() proxy is not revalidation-safe.
#[test]
fn back_from_drilled_in_detail_after_empty_revalidation_returns_to_list_not_quit() {
    let mut model = make_detail_model(make_rows(&["MINE-1"]), ListOrigin::Mine, MINE_JQL);
    model.revalidating = true;

    let (revalidated, _) = update(model, Msg::RevalidationLoaded(vec![], None));
    assert!(
        revalidated.rows.is_empty(),
        "revalidation legitimately swapped rows to empty while still on Detail"
    );

    let (next, cmds) = update(revalidated, Msg::Back);

    assert_eq!(
        next.screen,
        Screen::List,
        "a drilled-in detail's jql is non-empty, so Back must return to the list, not Quit"
    );
    assert!(cmds.is_empty());
}

#[test]
fn back_from_detail_with_rows_returns_to_the_mine_list_it_came_from() {
    let model = make_detail_model(make_rows(&["MINE-1"]), ListOrigin::Mine, MINE_JQL);

    let (next, cmds) = update(model, Msg::Back);

    assert_eq!(next.screen, Screen::List);
    assert!(next.detail.is_none());
    assert!(cmds.is_empty());
}

#[test]
fn back_from_detail_with_rows_and_search_origin_returns_to_the_search_list() {
    let model = make_detail_model(make_rows(&["NIKE-1"]), ListOrigin::Search, "project = NIKE");

    let (next, cmds) = update(model, Msg::Back);

    assert_eq!(next.screen, Screen::List);
    assert_eq!(next.list_origin, ListOrigin::Search);
    assert!(cmds.is_empty());
}

#[test]
fn back_from_list_with_project_origin_still_returns_to_projects() {
    let mut model = make_list_model(
        &["ALPHA-1"],
        ListOrigin::Project("ALPHA".to_owned()),
        "project = ALPHA ORDER BY updated DESC",
    );
    model.projects = vec![project_row("ALPHA", "Alpha Project")];

    let (next, cmds) = update(model, Msg::Back);

    assert_eq!(next.screen, Screen::Projects);
    assert!(cmds.is_empty());
}
