use crate::tracking;
use anyhow::{Context, Result};

/// Commands RTK can proxy — shim scripts are created for each of these.
/// Keep in sync with the Commands enum in main.rs.
const RTK_PROXIED_COMMANDS: &[&str] = &[
    "ls",
    "tree",
    "git",
    "gh",
    "grep",
    "find",
    "diff",
    "wc",
    "wget",
    "curl",
    "npm",
    "npx",
    "pnpm",
    "cargo",
    "aws",
    "psql",
    "docker",
    "kubectl",
    "vitest",
    "prisma",
    "tsc",
    "next",
    "prettier",
    "playwright",
    "ruff",
    "pytest",
    "pip",
    "go",
    "golangci-lint",
    "rake",
    "rspec",
    "rubocop",
    "dotnet",
    "gt",
];

/// Create a directory of shim scripts, one per RTK-proxied command.
/// Each shim: `#!/bin/sh\nexec /path/to/rtk <cmd> "$@"`.
/// Prepend this dir to PATH before running mise so that any command
/// executed by the task transparently goes through RTK.
fn create_rtk_shims(dir: &std::path::Path) -> Result<()> {
    let rtk_bin = std::env::current_exe().context("Failed to resolve current exe path")?;
    let rtk_path = rtk_bin.to_string_lossy();

    for &cmd in RTK_PROXIED_COMMANDS {
        let shim = dir.join(cmd);
        let content = format!("#!/bin/sh\nexec {rtk_path} {cmd} \"$@\"\n");
        std::fs::write(&shim, content)
            .with_context(|| format!("Failed to write shim for {cmd}"))?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&shim, std::fs::Permissions::from_mode(0o755))
                .with_context(|| format!("Failed to chmod shim for {cmd}"))?;
        }
    }
    Ok(())
}

/// Entry point: `rtk mise run <task> [extra-args...]`
///
/// RTK injects itself into the PATH used by mise before running the task.
/// When mise executes `ls -la` / `git status` / `npm install` / …, it finds
/// an RTK shim first, so the command output is already filtered.
/// Falls back to unmodified `mise` passthrough if shim creation fails.
pub fn run(args: &[String], verbose: u8) -> Result<()> {
    let timer = tracking::TimedExecution::start();

    let task_name = if args.first().map(String::as_str) == Some("run") {
        args.get(1).map(String::as_str).unwrap_or("")
    } else {
        args.first().map(String::as_str).unwrap_or("")
    };

    // Create shim dir and prepend to PATH.
    // RTK_ORIGINAL_PATH is set so that RTK invocations inside the shims
    // resolve real binaries (not shims) and avoid infinite recursion.
    let original_path = std::env::var("PATH").unwrap_or_default();
    let shim_dir = tempfile::tempdir().context("Failed to create shim tmpdir")?;
    let injected_path = match create_rtk_shims(shim_dir.path()) {
        Ok(()) => {
            Some(format!("{}:{original_path}", shim_dir.path().display()))
        }
        Err(e) => {
            if verbose > 0 {
                eprintln!("rtk mise: shim creation failed ({e}), running unfiltered");
            }
            None
        }
    };

    let mut cmd = std::process::Command::new("mise");
    for arg in args {
        cmd.arg(arg);
    }
    if let Some(path) = &injected_path {
        cmd.env("PATH", path);
        cmd.env("RTK_ORIGINAL_PATH", &original_path);
    }

    if verbose > 0 {
        eprintln!(
            "rtk mise: task={task_name:?}, shims={}",
            injected_path.is_some()
        );
    }

    let output = cmd
        .output()
        .with_context(|| format!("Failed to run mise {}", args.join(" ")))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Combine and strip the mise task echo header (e.g. "[status] $ git status")
    let raw = format!("{stdout}{stderr}");
    let filtered = strip_mise_headers(&raw);

    let orig_cmd = format!("mise run {task_name}");
    let rtk_cmd = format!("rtk mise run {task_name}");
    timer.track(&orig_cmd, &rtk_cmd, &raw, &filtered);

    print!("{filtered}");

    if !output.status.success() {
        std::process::exit(output.status.code().unwrap_or(1));
    }

    Ok(())
}

/// Strip mise task echo headers like `[taskname] $ command`.
fn strip_mise_headers(output: &str) -> String {
    lazy_static::lazy_static! {
        static ref MISE_HEADER: regex::Regex =
            regex::Regex::new(r"^\[[\w:/-]+\] \$ .+$").unwrap();
    }
    output
        .lines()
        .filter(|line| !MISE_HEADER.is_match(line.trim()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_mise_headers() {
        let input = "[status] $ git status\n* main\n~ Modified: 1 files\n   foo.rs";
        let result = strip_mise_headers(input);
        assert!(!result.contains("[status] $ git status"));
        assert!(result.contains("* main"));
        assert!(result.contains("foo.rs"));
    }

    #[test]
    fn test_strip_mise_headers_colon_task() {
        let input = "[build:release] $ cargo build --release\ncargo build (1 crates compiled)";
        let result = strip_mise_headers(input);
        assert!(!result.contains("[build:release]"));
        assert!(result.contains("cargo build (1 crates compiled)"));
    }

    #[test]
    fn test_strip_mise_headers_no_header() {
        let input = "* main\n~ Modified: 1 files";
        let result = strip_mise_headers(input);
        assert_eq!(result, input);
    }
}
