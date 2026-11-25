use std::fs;
use std::path::PathBuf;

use macfly_core::config::{Config, Preset};
use macfly_core::plan::PlanProposal;

fn extract_ts_const(source: &str, name: &str) -> String {
    let marker = format!("const {name} = `");
    let start = source
        .find(&marker)
        .unwrap_or_else(|| panic!("const `{name}` not found in demo.astro"))
        + marker.len();
    let rest = &source[start..];
    let end = rest
        .find("`;")
        .unwrap_or_else(|| panic!("closing backtick for `{name}` not found"));
    // Unescape template-literal escaped backslashes (e.g. \\n -> \n for JSON strings)
    rest[..end].replace("\\\\", "\\")
}

#[test]
fn demo_astro_json_examples_match_rust_schemas() {
    let demo_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../site/src/pages/demo.astro");
    if !demo_path.exists() {
        // When packaged on crates.io, site/ is excluded from the crate tarball.
        return;
    }
    let source = fs::read_to_string(&demo_path).expect("read demo.astro");

    let reorganize_json = extract_ts_const(&source, "reorganizePlanJson");
    let reorganize: PlanProposal =
        serde_json::from_str(&reorganize_json).expect("reorganizePlanJson must match PlanProposal");
    assert_eq!(reorganize.operations.len(), 3);

    let draft_json = extract_ts_const(&source, "draftReplyPlanJson");
    let draft: PlanProposal =
        serde_json::from_str(&draft_json).expect("draftReplyPlanJson must match PlanProposal");
    assert_eq!(draft.operations.len(), 1);

    let config_json = extract_ts_const(&source, "macflyConfigJson");
    let cfg: Config =
        serde_json::from_str(&config_json).expect("macflyConfigJson must match Config");
    assert_eq!(cfg.profiles["default"].preset, Preset::Balanced);
}
