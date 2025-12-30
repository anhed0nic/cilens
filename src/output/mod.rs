mod progress;
mod styling;
mod summary;
mod tables;

pub use progress::PhaseProgress;
pub use styling::{dim, magenta_bold};
pub use summary::print_summary;

// Banner
pub fn print_banner() {
    eprintln!(
        r"
{} {}
  {}
",
        magenta_bold("🔍 CILens"),
        dim(env!("CARGO_PKG_VERSION")),
        dim("CI/CD Insights Tool")
    );
}
