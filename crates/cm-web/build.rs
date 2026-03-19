fn main() {
    // Ensure frontend/dist/ exists so rust-embed compiles even before npm run build.
    let dist = std::path::Path::new("frontend/dist");
    if !dist.exists() {
        std::fs::create_dir_all(dist).ok();
    }
}
