//! `docs/project-commands.md` lists exactly the commands in `src/commands` (SON-P09): adding,
//! removing or renaming a command without updating the catalog fails here.

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

fn implemented() -> BTreeSet<String> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands");
    let mut names = BTreeSet::new();
    for entry in fs::read_dir(dir).expect("the commands folder") {
        let text = fs::read_to_string(entry.expect("an entry").path()).expect("a source file");
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("impl Command for ") {
                names.insert(rest.split_whitespace().next().expect("a name").to_owned());
            }
        }
    }
    names
}

fn documented() -> BTreeSet<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/project-commands.md");
    fs::read_to_string(path)
        .expect("docs/project-commands.md")
        .lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let name = cells.get(2)?.strip_prefix('`')?.strip_suffix('`')?;
            Some(name.to_owned())
        })
        .collect()
}

#[test]
fn the_command_catalog_matches_the_code() {
    let (code, docs) = (implemented(), documented());
    assert!(code.len() > 50, "found only {} commands", code.len());
    let undocumented: Vec<_> = code.difference(&docs).collect();
    let stale: Vec<_> = docs.difference(&code).collect();
    assert_eq!(
        (undocumented, stale),
        (vec![], vec![]),
        "update docs/project-commands.md"
    );
}
