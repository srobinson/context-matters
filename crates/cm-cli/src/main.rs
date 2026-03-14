use anyhow::Result;

fn main() -> Result<()> {
    println!("context-matters v{}", env!("CARGO_PKG_VERSION"));
    Ok(())
}
