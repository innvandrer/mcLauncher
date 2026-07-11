//! Creates a Windows desktop shortcut (.lnk) that relaunches EZMapa straight
//! into a specific world or server — "Quick Play" as a physical icon a user
//! (or a Stream Deck) can click, skipping the launcher UI and the game's main
//! menu entirely. Uses `WScript.Shell` via a short PowerShell script rather
//! than a COM/.lnk-writing crate, since Windows already ships PowerShell and
//! that avoids a new dependency for one feature.

use crate::error::{Error, Result};
#[cfg(windows)]
use std::path::PathBuf;

/// Escape a value for a single-quoted PowerShell string literal (the only
/// special character is `'`, doubled to escape).
#[cfg(any(windows, test))]
fn ps_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "''"))
}

/// Quote a value as one Windows command-line argument, following the same
/// backslash/quote rules the MSVC runtime uses to split argv — so
/// `std::env::args()` on the receiving end parses it back out intact.
#[cfg(any(windows, test))]
fn win_arg_quote(s: &str) -> String {
    if !s.is_empty() && !s.chars().any(|c| c == ' ' || c == '"' || c == '\t') {
        return s.to_string();
    }
    let mut out = String::from("\"");
    let mut backslashes = 0usize;
    for c in s.chars() {
        match c {
            '\\' => {
                backslashes += 1;
                out.push('\\');
            }
            '"' => {
                out.push_str(&"\\".repeat(backslashes + 1));
                out.push('"');
                backslashes = 0;
            }
            _ => {
                backslashes = 0;
                out.push(c);
            }
        }
    }
    out.push_str(&"\\".repeat(backslashes));
    out.push('"');
    out
}

#[cfg(windows)]
fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if "<>:\"/\\|?*".contains(c) { '_' } else { c })
        .collect();
    let trimmed = cleaned.trim().trim_end_matches('.').trim();
    if trimmed.is_empty() {
        "EZMapa".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Create a Desktop shortcut that relaunches `instance_name` straight into
/// `world` or `server` (or just opens EZMapa to that instance if both are
/// `None`). Returns the path to the created `.lnk`.
#[cfg(windows)]
pub fn create_desktop_shortcut(
    instance_id: &str,
    instance_name: &str,
    world: Option<&str>,
    server: Option<&str>,
) -> Result<String> {
    let exe = std::env::current_exe()
        .map_err(|e| Error::Other(format!("Could not locate EZMapa.exe: {e}")))?;
    let exe_dir = exe.parent().map(|p| p.to_path_buf()).unwrap_or_default();

    let mut args = format!("--quick-instance {}", win_arg_quote(instance_id));
    let suffix = if let Some(w) = world.filter(|s| !s.is_empty()) {
        args.push_str(&format!(" --quick-world {}", win_arg_quote(w)));
        format!(" - {w}")
    } else if let Some(s) = server.filter(|s| !s.is_empty()) {
        args.push_str(&format!(" --quick-server {}", win_arg_quote(s)));
        format!(" - {s}")
    } else {
        String::new()
    };

    let desktop = dirs::desktop_dir()
        .ok_or_else(|| Error::Other("Could not find the Desktop folder.".into()))?;
    let base_name = sanitize_filename(&format!("Play {instance_name}{suffix}"));
    let mut dest: PathBuf = desktop.join(format!("{base_name}.lnk"));
    let mut n = 2;
    while dest.exists() {
        dest = desktop.join(format!("{base_name} ({n}).lnk"));
        n += 1;
    }

    let script = format!(
        "$s = New-Object -ComObject WScript.Shell; \
         $sc = $s.CreateShortcut({dest}); \
         $sc.TargetPath = {exe}; \
         $sc.Arguments = {args}; \
         $sc.WorkingDirectory = {dir}; \
         $sc.IconLocation = {icon}; \
         $sc.Save()",
        dest = ps_quote(&dest.to_string_lossy()),
        exe = ps_quote(&exe.to_string_lossy()),
        args = ps_quote(&args),
        dir = ps_quote(&exe_dir.to_string_lossy()),
        icon = ps_quote(&format!("{},0", exe.to_string_lossy())),
    );

    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|e| Error::Other(format!("Could not run PowerShell: {e}")))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(Error::Other(format!(
            "Failed to create shortcut: {}",
            stderr.trim()
        )));
    }
    Ok(dest.to_string_lossy().to_string())
}

#[cfg(not(windows))]
pub fn create_desktop_shortcut(
    _instance_id: &str,
    _instance_name: &str,
    _world: Option<&str>,
    _server: Option<&str>,
) -> Result<String> {
    Err(Error::Other(
        "Desktop shortcuts are currently Windows-only.".into(),
    ))
}

/// Quick-launch request parsed from the process's command-line arguments
/// (set by a shortcut created via [`create_desktop_shortcut`]).
pub struct QuickLaunch {
    pub instance_id: String,
    pub world: Option<String>,
    pub server: Option<String>,
}

/// Parse `--quick-instance <id> [--quick-world <name> | --quick-server <addr>]`
/// out of the process argv (excluding argv\[0\], the exe path).
pub fn parse_quick_launch(args: &[String]) -> Option<QuickLaunch> {
    let mut instance_id = None;
    let mut world = None;
    let mut server = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--quick-instance" => instance_id = it.next().cloned(),
            "--quick-world" => world = it.next().cloned(),
            "--quick-server" => server = it.next().cloned(),
            _ => {}
        }
    }
    instance_id.map(|instance_id| QuickLaunch {
        instance_id,
        world,
        server,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win_arg_quote_leaves_simple_tokens_bare() {
        assert_eq!(win_arg_quote("survival-abc12345"), "survival-abc12345");
    }

    #[test]
    fn win_arg_quote_wraps_and_escapes_spaces_and_quotes() {
        assert_eq!(win_arg_quote(r#"My "Cool" World"#), r#""My \"Cool\" World""#);
    }

    #[test]
    fn win_arg_quote_doubles_trailing_backslashes_before_closing_quote() {
        assert_eq!(win_arg_quote(r"a b\"), r#""a b\\""#);
    }

    #[test]
    fn ps_quote_doubles_embedded_single_quotes() {
        assert_eq!(ps_quote("O'Brien's"), "'O''Brien''s'");
    }

    #[test]
    fn parse_quick_launch_reads_all_three_flags() {
        let args: Vec<String> = [
            "--quick-instance",
            "abc123",
            "--quick-world",
            "My World",
        ]
        .into_iter()
        .map(String::from)
        .collect();
        let ql = parse_quick_launch(&args).unwrap();
        assert_eq!(ql.instance_id, "abc123");
        assert_eq!(ql.world.as_deref(), Some("My World"));
        assert_eq!(ql.server, None);
    }

    #[test]
    fn parse_quick_launch_none_without_instance_flag() {
        let args: Vec<String> = ["--quick-world", "My World"].into_iter().map(String::from).collect();
        assert!(parse_quick_launch(&args).is_none());
    }
}
