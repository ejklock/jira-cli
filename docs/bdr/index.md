# BDRs

Observable-behavior decisions for `jira-cli`: Given/When/Then scenarios plus a
Test Design matrix. The slice Issue links the matrix rather than copying it.
Append-only: supersede or amend.

## Active

* [0001 — get an issue by key or URL — human render + agent_json](0001-get-issue-by-key.md) - Accepted
* [0002 — setup: add/list/remove/test Jira Cloud instances](0002-setup-instance-management.md) - Accepted
* [0003 — local-first cache read — offline get, --refresh re-fetch](0003-local-first-cache-read.md) - Accepted
* [0004 — current: derive the issue key from the git branch](0004-current-from-git-branch.md) - Accepted
* [0005 — mine/list and search — JQL issue listing](0005-mine-and-search-jql.md) - Accepted
* [0006 — browse TUI — interactive read-only navigation](0006-browse-tui-interactions.md) - Accepted
* [0007 — browse TUI visual design system — observable behaviors](0007-tui-visual-design-behaviors.md) - Accepted
* [0008 — Browse-entry SWR: paint the cached list instantly, always revalidate, guard the swap](0008-browse-entry-swr-behaviors.md) - Accepted
* [0009 — Browse TUI mouse: wheel navigates, card click drills in, nothing exits](0009-browse-mouse-interactions.md) - Accepted
* [0010 — Body links: inline visible '[url]', Ctrl/Cmd+click opens, plain click never navigates](0010-inline-body-link-behaviors.md) - Accepted
* [0011 — Detail text-selection behaviors (drag, highlight, copy, clear)](0011-detail-text-selection-behaviors.md) - Accepted
* [0012 — Issue attachments — agent_json array and inline detail panel behaviors](0012-attachments-behaviors.md) - Accepted
* [0013 — Projects axis — 'p' lists projects, Enter drills into the project's issues, back pops home](0013-projects-axis-behaviors.md) - Accepted
* [0014 — `jira comment` posts a comment to an issue as the logged-in user, from -m or stdin, with a --json write result](0014-non-interactive-comment-behaviors.md) - Accepted
* [0015 — Comment compose: 'c' opens a centered modal over a dimmed thread; multi-line typing; Ctrl+S posts and the thread reloads from the server; failures keep the draft](0015-comment-compose-behaviors.md) - Accepted
* [0016 — read commands open the browse TUI in an interactive terminal; agent mode prints](0016-interactive-default-read-commands.md) - Accepted
* [0017 — Comment actions on the browse detail — [ ] focus a comment, e edits your own (pre-filled compose), d deletes your own (Sim/Não confirm), r replies to anyone (mentioned new comment); every mutation reloads the thread from the server](0017-comment-action-behaviors.md) - Accepted
* [0018 — Status transition on the browse detail — s opens a transition picker fetched from the workflow, Enter moves the issue, the detail reloads from the server](0018-status-transition-behaviors.md) - Accepted
* [0019 — `jira skill` serves the embedded agent skill; the installer writes thin per-harness pointers with a project|global scope](0019-jira-skill-command-behaviors.md) - Accepted
* [0020 — Attachment download and external image viewer behaviors](0020-attachment-download-and-external-image-viewer-behaviors.md) - Proposed
