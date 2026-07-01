mod model;
mod shell;
mod view;

// `mod tests` (gated on cfg(test)) is the only consumer of these re-exports,
// so a plain `cargo build` sees them as unused.
#[allow(unused_imports)]
pub use model::{update, Cmd, Model, Msg, Screen};
pub use shell::browse;
#[allow(unused_imports)]
pub use view::{view, view_detail};

#[allow(unused_imports)]
pub(crate) use shell::{fetch_and_run, run_search};

#[cfg(test)]
#[path = "../../tests/unit/tui.rs"]
mod tests;
