use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let frontend_dir = manifest_dir.join("frontend");
    let index_html = manifest_dir.join("frontend/dist/index.html");

    track_frontend_inputs();

    run_pnpm(&frontend_dir, &["install", "--frozen-lockfile"]);
    run_pnpm(&frontend_dir, &["build"]);

    assert!(
        index_html.exists(),
        "frontend build completed without writing {}",
        index_html.display()
    );
}

fn track_frontend_inputs() {
    for path in [
        "frontend/package.json",
        "frontend/pnpm-lock.yaml",
        "frontend/pnpm-workspace.yaml",
        "frontend/index.html",
        "frontend/vite.config.ts",
        "frontend/tsconfig.json",
        "frontend/tsconfig.app.json",
        "frontend/tsconfig.node.json",
        "frontend/src",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
}

fn run_pnpm(frontend_dir: &Path, args: &[&str]) {
    let mut command = package_manager_command(args);
    let status = command
        .current_dir(frontend_dir)
        .status()
        .unwrap_or_else(|e| panic!("failed to run pnpm {}: {e}", args.join(" ")));

    assert!(
        status.success(),
        "pnpm {} failed with status {status}",
        args.join(" ")
    );
}

fn package_manager_command(args: &[&str]) -> Command {
    if command_exists("corepack") {
        let mut command = Command::new("corepack");
        command.arg("pnpm").args(args);
        return command;
    }

    let mut command = Command::new("pnpm");
    command.args(args);
    command
}

fn command_exists(program: &str) -> bool {
    Command::new(program).arg("--version").output().is_ok()
}
