use std::fs;
use std::path::Path;

#[test]
fn build_script_builds_frontend_assets_when_dist_is_missing() {
    let build_script = read_build_script();

    assert!(
        build_script.contains("frontend/dist/index.html"),
        "build script should require the embedded SPA entrypoint"
    );
    assert!(
        build_script.contains("pnpm") && build_script.contains("build"),
        "build script should build real frontend assets for clean Cargo installs"
    );
    assert!(
        !build_script.contains("create_dir_all(dist).ok()"),
        "build script must not satisfy rust-embed with an empty dist directory"
    );
    assert!(
        !build_script.contains("if index_html.exists()"),
        "build script should not trust ignored local dist leftovers"
    );
}

#[test]
fn changelog_notes_cm_web_command_surface() {
    let changelog = read_changelog();
    let unreleased = changelog
        .split("## [")
        .next()
        .expect("changelog should include an unreleased section");

    assert!(
        unreleased.contains("cm web --open"),
        "unreleased changelog should announce cm web --open"
    );
    assert!(
        unreleased.contains("cm-web"),
        "unreleased changelog should note the standalone cm-web surface"
    );
    assert!(
        !changelog.contains("cm-web --open"),
        "release copy should not recommend cm-web --open"
    );
}

fn read_changelog() -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let changelog_path = manifest_dir.join("../../CHANGELOG.md");
    fs::read_to_string(changelog_path).expect("changelog should be readable")
}

fn read_build_script() -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let build_script_path = manifest_dir.join("build.rs");
    fs::read_to_string(build_script_path).expect("cm-web build script should be readable")
}
