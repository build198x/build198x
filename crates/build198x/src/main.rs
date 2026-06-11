//! The `build198x` CLI — currently a stub that reports its name and version.
//! The CLI proper (subcommands, pipeline wiring) arrives in a later unit.

/// The name-and-version banner, split out from `main` so it is testable.
fn banner() -> String {
    format!("{} {}", env!("CARGO_PKG_NAME"), env!("CARGO_PKG_VERSION"))
}

fn main() {
    println!("{}", banner());
}

#[cfg(test)]
mod tests {
    use super::banner;

    #[test]
    fn banner_reports_name_and_version() {
        assert_eq!(banner(), format!("build198x {}", env!("CARGO_PKG_VERSION")));
    }
}
