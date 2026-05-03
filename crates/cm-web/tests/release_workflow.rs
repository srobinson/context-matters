use std::fs;
use std::path::Path;

#[test]
fn release_workflow_builds_frontend_before_dist_build() {
    let workflow = read_release_workflow();
    let package_manager = read_package_manager();
    let setup_pnpm = find_step(&workflow, "- name: Install pnpm");
    let prepare_pnpm = find_run(
        &workflow,
        &format!("corepack prepare {package_manager} --activate"),
    );
    let install_deps = find_run(&workflow, "pnpm install --frozen-lockfile");
    let build_frontend = find_run(&workflow, "pnpm build");
    let dist_build = find_run(
        &workflow,
        "dist build ${{ needs.plan.outputs.tag-flag }} --print=linkage",
    );

    assert!(setup_pnpm < prepare_pnpm);
    assert!(prepare_pnpm < install_deps);
    assert!(install_deps < build_frontend);
    assert!(build_frontend < dist_build);
}

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

fn read_release_workflow() -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow_path = manifest_dir.join("../../.github/workflows/release.yml");
    fs::read_to_string(workflow_path).expect("release workflow should be readable")
}

fn read_build_script() -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let build_script_path = manifest_dir.join("build.rs");
    fs::read_to_string(build_script_path).expect("cm-web build script should be readable")
}

fn read_package_manager() -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let package_json_path = manifest_dir.join("frontend/package.json");
    let package_json =
        fs::read_to_string(package_json_path).expect("frontend package.json should be readable");
    let value: serde_json::Value =
        serde_json::from_str(&package_json).expect("frontend package.json should be valid JSON");

    value
        .get("packageManager")
        .and_then(serde_json::Value::as_str)
        .expect("frontend package.json should pin packageManager")
        .to_owned()
}

fn find_step(workflow: &str, needle: &str) -> usize {
    workflow
        .find(needle)
        .unwrap_or_else(|| panic!("expected release workflow step: {needle}"))
}

fn find_run(workflow: &str, command: &str) -> usize {
    workflow
        .find(command)
        .unwrap_or_else(|| panic!("expected release workflow command: {command}"))
}
