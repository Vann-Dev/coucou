// Codex hook installation.
//
// Codex discovers lifecycle hooks in "$CODEX_HOME/hooks.json" (default
// "~/.codex/hooks.json") or inline under a [hooks] table in "config.toml".
// Coucou writes a dedicated "hooks.json" so the user's "config.toml" is never
// touched.
//
// The contract is the same as the Claude Code installer in hooks.rs: dated
// backup, merge without touching anybody else's hooks, show the diff, write only
// after an explicit click. The relay is the very same "coucou-hook" binary,
// invoked with "--agent codex" so Codex sessions land on the Codex pill.
//
// One Codex-specific wrinkle: non-managed hooks stay untrusted until the user
// reviews them with "/hooks" in the CLI, so the settings panel says so.

use std::path::PathBuf;

use serde_json::{json, Value};
use crate::hooks::{self, HookPreview, HookStatus};
use crate::{platform, settings};

/// Every Codex event the island understands, with the hook timeout in seconds.
/// Codex defaults to 600 s for most events; the relay exits within two seconds
/// for everything but a permission request, so the short timeouts only stop a
/// wedged relay from stalling a turn. Codex caps "SessionEnd" at three seconds.
pub const HOOK_EVENTS: &[(&str, u64)] = &[
    ("SessionStart", 10),
    ("SessionEnd", 3),
    ("UserPromptSubmit", 10),
    ("PreToolUse", 10),
    ("PostToolUse", 10),
    ("PermissionRequest", 120),
    ("Stop", 10),
    ("SubagentStart", 10),
    ("SubagentStop", 10),
];

/// The agent tag the relay forwards so events route to the Codex pill.
const AGENT: &str = "codex";

/// "$CODEX_HOME/hooks.json", or "~/.codex/hooks.json" when the variable is unset.
/// Mirrors Codex's own lookup order.
pub fn settings_path() -> PathBuf {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| platform::home_dir().join(".codex"))
        .join("hooks.json")
}

/// The relay command Codex runs. On Windows Codex goes through the shell, so the
/// path is quoted and its separators are forward slashes; on Unix the path is a
/// single-quoted shell word.
#[cfg(windows)]
fn hook_command(_event: &str) -> String {
    let exe = settings::hook_exe_path().to_string_lossy().replace('\\', "/");
    format!("\"{exe}\" --agent {AGENT}")
}

#[cfg(unix)]
fn hook_command(_event: &str) -> String {
    format!(
        "{} --agent {AGENT}",
        hooks::sh_quote(&settings::hook_exe_path().to_string_lossy())
    )
}

/// Reads "~/.codex/hooks.json". Only a missing file means "start from nothing";
/// anything we cannot parse is refused rather than treated as empty.
fn read_hooks() -> Result<Value, String> {
    let path = settings_path();
    match std::fs::read(&path) {
        Ok(bytes) => hooks::parse_settings(&bytes, &path.display().to_string()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(json!({})),
        Err(err) => Err(format!("Can't read {}: {err}", path.display())),
    }
}

fn read_hooks_lossy() -> Value {
    read_hooks().unwrap_or_else(|_| json!({}))
}

/// Hooks with Coucou's entries added; everything else is left untouched.
fn merged(existing: &Value) -> Value {
    hooks::merged_with(existing, HOOK_EVENTS, &hook_command)
}

fn backup_path() -> PathBuf {
    settings_path().with_file_name(format!("hooks.json.bak-{}", hooks::stamp()))
}

fn current_fingerprint() -> String {
    match std::fs::read(settings_path()) {
        Ok(bytes) => hooks::fingerprint(&bytes),
        Err(_) => hooks::fingerprint(b""),
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

pub fn status() -> HookStatus {
    let current = read_hooks_lossy();
    let installed = current
        .get("hooks")
        .and_then(Value::as_object)
        .map(|hooks| {
            hooks
                .values()
                .filter_map(Value::as_array)
                .flatten()
                .any(hooks::entry_is_ours)
        })
        .unwrap_or(false);
    let hook_path = settings::hook_exe_path();
    HookStatus {
        installed,
        settings_path: settings_path().to_string_lossy().to_string(),
        hook_ready: hook_path.exists(),
        hook_path: hook_path.to_string_lossy().to_string(),
    }
}

pub fn preview(install: bool) -> Result<HookPreview, String> {
    let current = read_hooks()?;
    let next = if install { merged(&current) } else { hooks::without_ours(&current) };
    Ok(HookPreview {
        diff: hooks::unified_diff(&hooks::pretty(&current), &hooks::pretty(&next)),
        backup: backup_path().to_string_lossy().to_string(),
        settings_path: settings_path().to_string_lossy().to_string(),
        fingerprint: current_fingerprint(),
    })
}

/// Writes the merged (or cleaned) hooks.json after taking a dated backup. Same
/// safety as the Claude installer: refuse if the file changed since the preview,
/// back up first, then write beside the target and rename over it.
pub fn write(install: bool, fingerprint: &str) -> Result<String, String> {
    let path = settings_path();
    let dir = path.parent().unwrap_or(std::path::Path::new("."));
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;

    let current = read_hooks()?;
    if current_fingerprint() != fingerprint {
        return Err(format!(
            "{} changed since the preview. Nothing was written — review the new diff.",
            path.display()
        ));
    }

    let backup = backup_path();
    if path.exists() {
        std::fs::copy(&path, &backup).map_err(|e| format!("backup failed: {e}"))?;
    }

    let next = if install { merged(&current) } else { hooks::without_ours(&current) };
    let mut text = hooks::pretty(&next);
    text.push('\n');

    #[cfg(unix)]
    let path = std::fs::canonicalize(&path).unwrap_or(path);

    let temp = path.with_extension(format!("json.coucou-{}", std::process::id()));
    if let Err(err) = hooks::write_like(&temp, &path, text.as_bytes()) {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("write failed: {err}"));
    }
    if let Err(err) = std::fs::rename(&temp, &path) {
        let _ = std::fs::remove_file(&temp);
        return Err(format!("write failed: {err}"));
    }
    Ok(backup.to_string_lossy().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hooks_are_added_and_other_entries_survive() {
        let existing = json!({
            "description": "my hooks",
            "hooks": {
                "Stop": [
                    { "hooks": [{ "type": "command", "command": "echo someone-else" }] }
                ]
            }
        });
        let after = merged(&existing);
        assert_eq!(after["description"], "my hooks");
        let stop = after["hooks"]["Stop"].as_array().unwrap();
        assert!(stop
            .iter()
            .any(|e| serde_json::to_string(e).unwrap().contains("someone-else")));
        assert!(stop.iter().any(hooks::entry_is_ours), "coucou hook not added");
        assert!(after["hooks"]["PermissionRequest"].is_array());

        // Removing ours restores the original exactly.
        assert_eq!(hooks::without_ours(&after), existing);
    }

    #[test]
    fn the_installed_shape_matches_what_codex_reads() {
        let after = merged(&json!({}));
        let handler = &after["hooks"]["PermissionRequest"][0]["hooks"][0];
        assert_eq!(handler["type"], "command");
        // The relay waits up to 110 s for a human, so Codex must allow longer.
        assert_eq!(handler["timeout"], 120);
        // Paths differ per machine; the agent tag must not.
        assert!(handler["command"].as_str().unwrap().contains("--agent codex"));
        // Codex caps SessionEnd at three seconds.
        assert_eq!(after["hooks"]["SessionEnd"][0]["hooks"][0]["timeout"], 3);
    }
}
