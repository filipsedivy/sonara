//! Writes `crates/sonara-app/src/composer/system_prompt_orchestra.md`, the orchestration
//! reference that the AI Chat's agents receive after the composer's system prompt, as
//! `sonara_app::instruments::agent_reference` renders it from the orchestration metadata, the
//! library profiles Sonara ships and the guide
//! (`crates/sonara-app/src/instruments/reference_guide.md`). Run it after changing any of them:
//!
//! ```text
//! cargo run -p sonara-app --example gen_chat_reference
//! ```
//!
//! `cargo test` runs this example's test, which fails while the committed file differs from the
//! rendering, so every CI leg and `scripts/dev.sh rust-check` check that the prompt is up to
//! date. The composer includes the file as it includes `system_prompt.md`, because the chat's
//! privacy guard keeps it from reaching the metadata or the catalog itself.

use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use sonara_app::instruments::agent_reference;

/// The generated file, relative to the crate's manifest directory.
const OUTPUT: &str = "src/composer/system_prompt_orchestra.md";

fn main() -> ExitCode {
    if std::env::args().len() > 1 {
        eprintln!("usage: cargo run -p sonara-app --example gen_chat_reference");
        return ExitCode::from(2);
    }
    let text = agent_reference();
    match fs::write(output_path(), &text) {
        Ok(()) => {
            println!(
                "wrote crates/sonara-app/{OUTPUT} ({} characters)",
                text.chars().count()
            );
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("gen_chat_reference: cannot write {OUTPUT}: {err}");
            ExitCode::FAILURE
        }
    }
}

/// The path of [`OUTPUT`], from the crate's manifest directory (`crates/sonara-app`).
fn output_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(OUTPUT)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use sonara_app::composer::invocation::ORCHESTRATION_REFERENCE;
    use sonara_app::instruments::agent_reference;

    use super::{OUTPUT, output_path};

    /// The committed file is what the sources render, so a change of the metadata, a profile or
    /// the guide that was not regenerated, or a hand edit of the file, fails the build (DOC-01).
    #[test]
    fn the_committed_chat_reference_is_up_to_date() {
        let committed = fs::read_to_string(output_path()).expect(OUTPUT);
        assert!(
            committed == agent_reference(),
            "{OUTPUT} is out of date: run `cargo run -p sonara-app --example gen_chat_reference`"
        );
    }

    /// What the composer sends is that file, as this build compiled it.
    #[test]
    fn the_composer_sends_the_committed_file() {
        let committed = fs::read_to_string(output_path()).expect(OUTPUT);
        assert_eq!(ORCHESTRATION_REFERENCE, committed);
    }
}
