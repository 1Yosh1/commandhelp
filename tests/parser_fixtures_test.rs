// tests/parser_fixtures_test.rs
//
// Spec §8.1 asks for coverage against 30+ real-world `--help` texts. Every file
// in `tests/fixtures` is parsed here, so adding a fixture widens the corpus
// automatically; a fixture that yields nothing is a regression, not coverage.

use chelp::parser::parse_help_output;
use std::path::Path;

#[test]
fn test_every_fixture_parses_into_something_useful() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests").join("fixtures");
    let mut parsed = 0usize;
    let mut with_flags = 0usize;
    let mut with_subcommands = 0usize;
    let mut failures: Vec<String> = Vec::new();

    let entries = std::fs::read_dir(&dir).expect("read fixtures dir");
    for entry in entries {
        let path = entry.expect("entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("txt") {
            continue;
        }
        let file_name = path.file_name().unwrap().to_string_lossy().to_string();
        let binary = file_name.trim_end_matches("_help.txt");
        let text = std::fs::read_to_string(&path).expect("read fixture");

        let schema = parse_help_output(binary, &[], &text)
            .unwrap_or_else(|e| panic!("{} failed to parse: {}", file_name, e));

        assert_eq!(schema.binary, binary, "{}: wrong binary stamped", file_name);
        if schema.flags.is_empty() && schema.subcommands.is_empty() {
            failures.push(file_name);
            continue;
        }

        parsed += 1;
        if !schema.flags.is_empty() {
            with_flags += 1;
        }
        if !schema.subcommands.is_empty() {
            with_subcommands += 1;
        }
    }

    assert!(
        failures.is_empty(),
        "fixtures parsed to zero flags and zero subcommands: {:?}",
        failures
    );

    // Spec §8.1: 30+ fixtures, and the corpus must exercise both styles.
    assert!(
        parsed >= 30,
        "spec asks for 30+ fixtures, found {}",
        parsed
    );
    assert!(
        with_flags >= 20,
        "expected most fixtures to yield flags, only {} did",
        with_flags
    );
    assert!(
        with_subcommands >= 10,
        "expected several fixtures to yield subcommands, only {} did",
        with_subcommands
    );
}
