//! `--log-level`: the prototype's `logging.basicConfig(level=LEVEL.upper(), stream=sys.stderr,
//! format="%(levelname)s %(name)s: %(message)s")` with `env_logger`.
//!
//! Python's level names map onto `log`'s filters ([`sonara_app::log_level`], shared with the
//! desktop application). The logger name is the record's target: a Rust module path
//! (`sonara_agent::registry`) where the prototype names its Python module, unless the code sets
//! a target (`sonara.mcp`).

use std::io::Write as _;

use sonara_app::log_level::{level_filter, level_name};

/// Installs the stderr logger at `name`'s level. Like `basicConfig`, a second call changes
/// nothing (`sonara mcp --log-level` keeps the level of `sonara --log-level`).
///
/// # Errors
///
/// `Unknown level: 'NAME'` (Python's `ValueError`) for an unknown level name.
pub(crate) fn init(name: &str) -> Result<(), String> {
    let filter = level_filter(name).map_err(|error| error.to_string())?;
    // ERR-06: an error only means that a logger is installed already, and `basicConfig` does
    // nothing then either.
    let _ = env_logger::Builder::new()
        .filter_level(filter)
        .target(env_logger::Target::Stderr)
        .format(|out, record| {
            writeln!(
                out,
                "{} {}: {}",
                level_name(record.level()),
                record.target(),
                record.args()
            )
        })
        .try_init();
    Ok(())
}
