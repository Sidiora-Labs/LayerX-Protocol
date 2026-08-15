use std::env;
use std::fs;
use std::path::{Path, PathBuf};

const BANNED_DEPENDENCIES: &[&str] = &[
    "bindgen",
    "libsqlite3-sys",
    "rusqlite",
    "sqlx-sqlite",
    "layerx-core",
    "layerx-protocol-core",
];

const PRIVATE_NODE_PATHS: &[&str] = &[
    "node.db",
    "projection.db",
    "append-only.log",
    "log/segments/",
    "state/snapshots/",
    "snapshot.bin",
];

const AMBIENT_NONDETERMINISM: &[&str] = &[
    "SystemTime::now",
    "Instant::now",
    "thread_rng(",
    "getrandom(",
    "OsRng",
];

#[derive(Debug, Eq, PartialEq)]
struct Violation {
    path: PathBuf,
    rule: &'static str,
}

fn visit_files(root: &Path, output: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries =
        fs::read_dir(root).map_err(|error| format!("cannot read {}: {error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|name| name == "target") {
                continue;
            }
            visit_files(&path, output)?;
        } else if path.file_name().is_some_and(|name| name == "Cargo.toml")
            || path.extension().is_some_and(|extension| extension == "rs")
        {
            output.push(path);
        }
    }
    Ok(())
}

fn manifest_uses_banned_dependency(body: &str) -> bool {
    body.lines().any(|line| {
        let declaration = line.split('#').next().unwrap_or("").trim();
        BANNED_DEPENDENCIES.iter().any(|dependency| {
            declaration
                .strip_prefix(dependency)
                .is_some_and(|tail| tail.trim_start().starts_with('='))
        }) || declaration.starts_with("links = \"layerx")
    })
}

fn source_violation(body: &str) -> Option<&'static str> {
    if body.contains("include!(") && body.contains("include/layerx")
        || body.contains("include_bytes!(") && body.contains("include/layerx")
        || body.contains("bindgen::")
        || body.contains("#[link(name = \"layerx")
        || body.contains("extern \"C\"")
    {
        return Some("private-c-boundary");
    }
    if body.contains("#[repr(C)]") || body.contains("#[repr(C,") {
        return Some("mirrored-c-layout");
    }
    if PRIVATE_NODE_PATHS
        .iter()
        .any(|private| body.contains(private))
    {
        return Some("node-private-path");
    }
    if AMBIENT_NONDETERMINISM
        .iter()
        .any(|ambient| body.contains(ambient))
    {
        return Some("ambient-nondeterminism");
    }
    if body.contains("lni::transport::Uds")
        || body.contains("lni::transport::Tls")
        || body.contains("lni::abi::Handle")
    {
        return Some("direct-core-connection");
    }
    None
}

fn human_dependency_policy(root: &Path) -> Result<(), Vec<Violation>> {
    let mut files = Vec::new();
    if visit_files(root, &mut files).is_err() {
        return Err(vec![Violation {
            path: root.to_path_buf(),
            rule: "unreadable-human-tree",
        }]);
    }
    let violations = files
        .into_iter()
        .filter(|path| path.file_name().is_some_and(|name| name == "Cargo.toml"))
        .filter_map(|path| {
            let body = fs::read_to_string(&path).ok()?;
            manifest_uses_banned_dependency(&body).then_some(Violation {
                path,
                rule: "forbidden-dependency",
            })
        })
        .collect::<Vec<_>>();
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

fn human_boundary_policy(root: &Path) -> Result<(), Vec<Violation>> {
    let mut files = Vec::new();
    if visit_files(root, &mut files).is_err() {
        return Err(vec![Violation {
            path: root.to_path_buf(),
            rule: "unreadable-human-tree",
        }]);
    }
    let violations = files
        .into_iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .filter_map(|path| {
            let body = fs::read_to_string(&path).ok()?;
            source_violation(&body).map(|rule| Violation { path, rule })
        })
        .collect::<Vec<_>>();
    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

fn report(result: Result<(), Vec<Violation>>) -> bool {
    match result {
        Ok(()) => true,
        Err(violations) => {
            for violation in violations {
                eprintln!("{}: {}", violation.rule, violation.path.display());
            }
            false
        }
    }
}

fn main() {
    let root = env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("human/crates"), PathBuf::from);
    if !report(human_dependency_policy(&root)) || !report(human_boundary_policy(&root)) {
        std::process::exit(1);
    }
    println!("human dependency and boundary policies passed");
}

#[cfg(test)]
mod tests {
    use super::{human_boundary_policy, human_dependency_policy};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT: AtomicUsize = AtomicUsize::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir()
                .join(format!("layerx-human-boundary-{}-{id}", std::process::id()));
            fs::create_dir_all(path.join("sample/src"))
                .unwrap_or_else(|error| panic!("create fixture: {error}"));
            fs::write(
                path.join("sample/Cargo.toml"),
                "[package]\nname = \"sample\"\nversion = \"0.1.0\"\n",
            )
            .unwrap_or_else(|error| panic!("write manifest: {error}"));
            fs::write(path.join("sample/src/lib.rs"), "pub struct Value(u64);\n")
                .unwrap_or_else(|error| panic!("write source: {error}"));
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn clean_tree_passes_both_policies() {
        let fixture = Fixture::new();
        assert_eq!(human_dependency_policy(fixture.path()), Ok(()));
        assert_eq!(human_boundary_policy(fixture.path()), Ok(()));
    }

    #[test]
    fn sqlite_dependency_is_rejected() {
        let fixture = Fixture::new();
        fs::write(
            fixture.path().join("sample/Cargo.toml"),
            "[dependencies]\nrusqlite = \"1\"\n",
        )
        .unwrap_or_else(|error| panic!("write violation: {error}"));
        assert!(human_dependency_policy(fixture.path()).is_err());
    }

    #[test]
    fn node_private_paths_are_rejected() {
        let fixture = Fixture::new();
        fs::write(
            fixture.path().join("sample/src/lib.rs"),
            "const DATABASE: &str = \"projection.db\";\n",
        )
        .unwrap_or_else(|error| panic!("write violation: {error}"));
        assert!(human_boundary_policy(fixture.path()).is_err());
    }

    #[test]
    fn ambient_clock_reads_are_rejected() {
        let fixture = Fixture::new();
        fs::write(
            fixture.path().join("sample/src/lib.rs"),
            "fn resolve() { let _ = std::time::SystemTime::now(); }\n",
        )
        .unwrap_or_else(|error| panic!("write violation: {error}"));
        assert!(human_boundary_policy(fixture.path()).is_err());
    }
}
