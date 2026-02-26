/// Self-update: checks the latest GitHub release and replaces the running binary.
///
/// Flow:
///   1. GET https://api.github.com/repos/natekali/itk/releases/latest  (no auth needed)
///   2. Parse "tag_name" → latest version string
///   3. Compare with env!("CARGO_PKG_VERSION")
///   4. If newer: download the platform-specific asset, replace current binary, done.
///   5. If already up-to-date: print message and exit 0.
///
/// No extra crates needed — uses only std::net via the `ureq`-free approach with
/// std::process + PowerShell/curl for the actual download, keeping the binary small.
/// Heavy HTTP work is delegated to the OS tools already available everywhere.

use std::env;
use std::io::{self, Write};

const REPO: &str = "natekali/itk";
const API_URL: &str = "https://api.github.com/repos/natekali/itk/releases/latest";

/// Detect the asset name for this platform/arch.
fn asset_name() -> &'static str {
    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return "itk-windows-x86_64.exe";

    #[cfg(all(target_os = "macos"))]
    return "itk-macos-universal";

    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return "itk-linux-x86_64";

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return "itk-linux-aarch64";

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "linux"
    )))]
    return "itk-linux-x86_64"; // fallback
}

pub fn run(check_only: bool) {
    let current = env!("CARGO_PKG_VERSION");

    eprint!("itk: checking for updates... ");
    let _ = io::stderr().flush();

    // Fetch latest release tag via GitHub API
    let latest = match fetch_latest_tag() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("failed.\nitk: update check failed: {e}");
            return;
        }
    };

    // Strip leading 'v' for comparison
    let latest_clean = latest.trim_start_matches('v');
    let current_clean = current.trim_start_matches('v');

    if latest_clean == current_clean {
        eprintln!("already up to date (v{current_clean}).");
        return;
    }

    eprintln!("v{current_clean} → {latest} available.");

    if check_only {
        eprintln!(
            "itk: run 'itk update' to install, or:\n  irm https://raw.githubusercontent.com/{REPO}/main/install.ps1 | iex"
        );
        return;
    }

    // Download and replace
    let asset = asset_name();
    let download_url = format!(
        "https://github.com/{REPO}/releases/download/{latest}/{asset}"
    );

    eprintln!("itk: downloading {asset} from {latest}...");

    if let Err(e) = download_and_replace(&download_url, asset) {
        eprintln!("itk: update failed: {e}");
        eprintln!("itk: manual install: irm https://raw.githubusercontent.com/{REPO}/main/install.ps1 | iex");
    } else {
        eprintln!("itk: updated to {latest}. Run 'itk --version' to confirm.");
    }
}

/// Fetch the latest release tag from GitHub API using the platform's HTTP tool.
/// Returns the raw tag string e.g. "v0.1.2"
fn fetch_latest_tag() -> Result<String, String> {
    let output = fetch_url_body(API_URL)?;

    // Parse "tag_name" from JSON without a full JSON parser
    // Response looks like: {"tag_name":"v0.1.2","name":"ITK v0.1.2",...}
    parse_json_string_field(&output, "tag_name")
        .ok_or_else(|| "could not find tag_name in GitHub API response".to_string())
}

/// Download a URL to a temp file, then atomically replace the current binary.
fn download_and_replace(url: &str, _asset: &str) -> Result<(), String> {
    let current_exe = env::current_exe()
        .map_err(|e| format!("cannot locate current binary: {e}"))?;

    // Temp path next to the current binary (same filesystem → atomic rename)
    let tmp_path = current_exe.with_extension("tmp");

    // Download to temp file
    download_to_file(url, &tmp_path)?;

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&tmp_path)
            .map_err(|e| e.to_string())?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&tmp_path, perms).map_err(|e| e.to_string())?;
    }

    // On Windows, we can't replace a running exe directly.
    // Strategy: rename current to .old, rename tmp to current, delete .old.
    #[cfg(windows)]
    {
        let old_path = current_exe.with_extension("old");
        // Remove stale .old if exists
        let _ = std::fs::remove_file(&old_path);
        // Move current → .old
        std::fs::rename(&current_exe, &old_path)
            .map_err(|e| format!("cannot move current binary: {e}"))?;
        // Move tmp → current
        std::fs::rename(&tmp_path, &current_exe)
            .map_err(|e| format!("cannot move new binary: {e}"))?;
        // Schedule .old for deletion (best-effort)
        let _ = std::fs::remove_file(&old_path);
    }

    // On Unix: direct rename is atomic
    #[cfg(unix)]
    {
        std::fs::rename(&tmp_path, &current_exe)
            .map_err(|e| format!("cannot replace binary: {e}"))?;
    }

    Ok(())
}

// ── Platform HTTP helpers ─────────────────────────────────────────────────────

/// Fetch URL body as string using platform-native HTTP tool.
fn fetch_url_body(url: &str) -> Result<String, String> {
    #[cfg(windows)]
    {
        fetch_with_powershell(url)
    }
    #[cfg(not(windows))]
    {
        fetch_with_curl(url)
    }
}

/// Download URL to a file path using platform-native HTTP tool.
fn download_to_file(url: &str, dest: &std::path::Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        download_with_powershell(url, dest)
    }
    #[cfg(not(windows))]
    {
        download_with_curl(url, dest)
    }
}

#[cfg(windows)]
fn fetch_with_powershell(url: &str) -> Result<String, String> {
    let script = format!(
        "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; \
         $r = Invoke-WebRequest -Uri '{url}' -UseBasicParsing \
              -Headers @{{\"User-Agent\"=\"itk/{ver}\"}}; \
         $r.Content",
        url = url,
        ver = env!("CARGO_PKG_VERSION")
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| format!("powershell not available: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(windows)]
fn download_with_powershell(url: &str, dest: &std::path::Path) -> Result<(), String> {
    let dest_str = dest.to_string_lossy();
    let script = format!(
        "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; \
         Invoke-WebRequest -Uri '{url}' -OutFile '{dest}' -UseBasicParsing \
              -Headers @{{\"User-Agent\"=\"itk/{ver}\"}}",
        url = url,
        dest = dest_str,
        ver = env!("CARGO_PKG_VERSION")
    );
    let out = std::process::Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .map_err(|e| format!("powershell not available: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(())
}

#[cfg(not(windows))]
fn fetch_with_curl(url: &str) -> Result<String, String> {
    let out = std::process::Command::new("curl")
        .args([
            "-fsSL",
            "-A", &format!("itk/{}", env!("CARGO_PKG_VERSION")),
            url,
        ])
        .output()
        .map_err(|e| format!("curl not available: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

#[cfg(not(windows))]
fn download_with_curl(url: &str, dest: &std::path::Path) -> Result<(), String> {
    let out = std::process::Command::new("curl")
        .args([
            "-fsSL",
            "-A", &format!("itk/{}", env!("CARGO_PKG_VERSION")),
            "-o", &dest.to_string_lossy(),
            url,
        ])
        .output()
        .map_err(|e| format!("curl not available: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(())
}

// ── Minimal JSON field extractor ──────────────────────────────────────────────

/// Extract a string field value from a flat JSON object without a full parser.
/// e.g. parse_json_string_field(r#"{"tag_name":"v0.1.2"}"#, "tag_name") → Some("v0.1.2")
fn parse_json_string_field(json: &str, field: &str) -> Option<String> {
    let needle = format!("\"{}\"", field);
    let start = json.find(&needle)?;
    let after_key = &json[start + needle.len()..];
    // Skip whitespace and ':'
    let after_colon = after_key.trim_start().trim_start_matches(':').trim_start();
    if !after_colon.starts_with('"') {
        return None;
    }
    let inner = &after_colon[1..];
    let end = inner.find('"')?;
    Some(inner[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::parse_json_string_field;

    #[test]
    fn test_parse_tag_name() {
        let json = r#"{"url":"https://api.github.com/repos/natekali/itk/releases/1","tag_name":"v0.1.2","name":"ITK v0.1.2","draft":false}"#;
        assert_eq!(
            parse_json_string_field(json, "tag_name"),
            Some("v0.1.2".to_string())
        );
    }

    #[test]
    fn test_parse_missing_field() {
        let json = r#"{"name":"ITK v0.1.2"}"#;
        assert_eq!(parse_json_string_field(json, "tag_name"), None);
    }
}
