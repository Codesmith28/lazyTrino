// Copyright 2026 Sarthak Siddhpura
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{
    env,
    fs::{self, File},
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

pub const REPO: &str = "Codesmith28/lazyTrino";
pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub html_url: Option<String>,
    pub assets: Vec<ReleaseAsset>,
}

/// Parses semver-like strings such as "v0.1.0", "0.2.1", "1.0.0-rc1" into (major, minor, patch).
pub fn parse_version(raw: &str) -> Option<(u64, u64, u64)> {
    let s = raw.trim().trim_start_matches('v').trim_start_matches('V');
    let core = s.split(['-', '+']).next()?;
    let parts: Vec<&str> = core.split('.').collect();
    if parts.len() < 2 {
        return None;
    }
    let major = parts[0].parse::<u64>().ok()?;
    let minor = parts[1].parse::<u64>().ok()?;
    let patch = if parts.len() >= 3 {
        parts[2].parse::<u64>().ok().unwrap_or(0)
    } else {
        0
    };
    Some((major, minor, patch))
}

/// Compares two semver versions and returns true if `latest` is strictly newer than `current`.
pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some((l_maj, l_min, l_pat)), Some((c_maj, c_min, c_pat))) => {
            (l_maj, l_min, l_pat) > (c_maj, c_min, c_pat)
        }
        _ => latest != current && !latest.is_empty(),
    }
}

/// Detects the current OS and architecture strings.
pub fn detect_target() -> (&'static str, &'static str) {
    let os = match env::consts::OS {
        "linux" => "linux",
        "macos" => "darwin",
        other => other,
    };
    let arch = env::consts::ARCH;
    (os, arch)
}

/// Matches the best release asset for the given OS and architecture.
pub fn match_release_asset<'a>(
    assets: &'a [ReleaseAsset],
    os: &str,
    arch: &str,
) -> Option<&'a ReleaseAsset> {
    let target_triple = match (os, arch) {
        ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
        ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
        ("darwin", "x86_64") => "x86_64-apple-darwin",
        ("darwin", "aarch64") => "aarch64-apple-darwin",
        _ => "",
    };

    let alt_target = format!("{os}-{arch}");
    let alt_arch = if arch == "x86_64" {
        "amd64"
    } else if arch == "aarch64" {
        "arm64"
    } else {
        arch
    };
    let alt_target2 = format!("{os}-{alt_arch}");

    // 1. Exact triple match in filename
    if !target_triple.is_empty()
        && let Some(asset) = assets.iter().find(|a| a.name.contains(target_triple))
    {
        return Some(asset);
    }

    // 2. Exact os-arch match
    if let Some(asset) = assets.iter().find(|a| a.name.contains(&alt_target)) {
        return Some(asset);
    }

    // 3. Alternate arch match (amd64 / arm64)
    if let Some(asset) = assets.iter().find(|a| a.name.contains(&alt_target2)) {
        return Some(asset);
    }

    // 4. Loose match: contains os AND (arch or alt_arch)
    assets.iter().find(|a| {
        let name = a.name.to_lowercase();
        name.contains(os) && (name.contains(arch) || name.contains(alt_arch))
    })
}

pub const LOGO_BANNER: &str = r#"  _                      _____     _             
 | |                    |_   _|   (_)            
 | | __ _ _____   _ ______| | _ __ _ _ __   ___  
 | |/ _` |_  / | | |______| || '__| | '_ \ / _ \ 
 | | (_| |/ /| |_| |      | || |  | | | | | (_) |
 |_|\__,_/___|\__, |      \_/|_|  |_|_| |_|\___/ 
               __/ |                             
              |___/                              "#;

fn print_logo() {
    print!("\x1b[1;36m{}\x1b[0m\n\n", LOGO_BANNER);
}

/// Atomically replaces the current binary with the newly downloaded binary.
pub fn replace_binary(temp_path: &Path, target_path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Ok(metadata) = fs::metadata(temp_path) {
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            let _ = fs::set_permissions(temp_path, perms);
        }
    }

    if let Err(err) = fs::rename(temp_path, target_path) {
        if err.kind() == io::ErrorKind::PermissionDenied {
            bail!(
                "Permission denied updating {}\nPlease run with elevated permissions:\n  sudo lazyTrino --update",
                target_path.display()
            );
        }
        // Fallback for cross-device renames (EXDEV)
        fs::copy(temp_path, target_path).context("Failed to copy updated binary to target path")?;
        let _ = fs::remove_file(temp_path);
    }

    Ok(())
}

fn extract_binary_from_archive(archive_path: &Path, dest_binary: &Path) -> Result<()> {
    // Check if the file is a gzip archive
    let mut file = File::open(archive_path).context("Failed to open downloaded archive")?;
    let mut magic = [0u8; 2];
    let is_gzip = io::Read::read_exact(&mut file, &mut magic).is_ok() && magic == [0x1f, 0x8b];
    drop(file);

    if is_gzip {
        let status = std::process::Command::new("tar")
            .arg("-xzf")
            .arg(archive_path)
            .arg("-C")
            .arg(archive_path.parent().unwrap_or(Path::new(".")))
            .status()
            .context("Failed to execute tar command to extract release package")?;

        if !status.success() {
            bail!("tar command failed to extract release package");
        }

        // Locate extracted lazyTrino executable
        let parent = archive_path.parent().unwrap_or(Path::new("."));
        let candidate = parent.join("lazyTrino");
        if candidate.exists() {
            if candidate != dest_binary {
                fs::rename(&candidate, dest_binary).context("Failed to place extracted binary")?;
            }
            return Ok(());
        }
    }

    // Direct binary: move archive_path to dest_binary
    if archive_path != dest_binary {
        fs::rename(archive_path, dest_binary)
            .or_else(|_| fs::copy(archive_path, dest_binary).map(|_| ()))
            .context("Failed to prepare target binary from downloaded file")?;
    }

    Ok(())
}

/// Queries the GitHub latest release web redirect (https://github.com/{REPO}/releases/latest)
/// to determine the latest release tag without hitting API rate limits.
pub async fn fetch_latest_tag_from_redirect(client: &reqwest::Client) -> Result<String> {
    let redirect_url = format!("https://github.com/{REPO}/releases/latest");
    let response = client
        .get(&redirect_url)
        .send()
        .await
        .context("Failed to query GitHub latest release redirect")?;

    let final_url = response.url().as_str();
    if let Some(tag_part) = final_url.split("/releases/tag/").nth(1) {
        let tag = tag_part.split(['?', '#']).next().unwrap_or(tag_part).trim();
        if !tag.is_empty() {
            return Ok(tag.to_string());
        }
    }
    bail!("Could not resolve latest release tag from redirect URL: {final_url}")
}

/// Executes the self-update workflow with rich, decorated progress output.
pub async fn run_self_update() -> Result<()> {
    print_logo();

    let (os, arch) = detect_target();
    println!(
        "\x1b[1;36m➜\x1b[0m Checking for updates (current version: \x1b[1mv{}\x1b[0m)...",
        CURRENT_VERSION
    );

    let client = reqwest::Client::builder()
        .user_agent(format!("lazyTrino/{CURRENT_VERSION}"))
        .timeout(std::time::Duration::from_secs(15))
        .build()
        .context("Failed to initialize HTTP client")?;

    let auth_token = env::var("GITHUB_TOKEN")
        .or_else(|_| env::var("GH_TOKEN"))
        .ok();
    let api_url = format!("https://api.github.com/repos/{REPO}/releases/latest");
    let mut req = client.get(&api_url);
    if let Some(ref token) = auth_token {
        req = req.header("Authorization", format!("Bearer {token}"));
    }

    let (latest_tag, asset) = match req.send().await {
        Ok(res) if res.status().is_success() => {
            let release: GithubRelease = res
                .json()
                .await
                .context("Failed to parse GitHub release metadata")?;
            let tag = release.tag_name.trim().to_string();
            let matched = match_release_asset(&release.assets, os, arch).cloned();
            (tag, matched)
        }
        _ => {
            // Fallback to GitHub web redirect on releases/latest (bypasses all API rate limits)
            let tag = fetch_latest_tag_from_redirect(&client).await?;
            let triple = match (os, arch) {
                ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
                ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
                ("darwin", "x86_64") => "x86_64-apple-darwin",
                ("darwin", "aarch64") => "aarch64-apple-darwin",
                _ => "",
            };
            let matched = if triple.is_empty() {
                None
            } else {
                let name = format!("lazyTrino-{tag}-{triple}.tar.gz");
                let url = format!("https://github.com/{REPO}/releases/download/{tag}/{name}");
                Some(ReleaseAsset {
                    name,
                    browser_download_url: url,
                    size: None,
                })
            };
            (tag, matched)
        }
    };

    if !is_newer(&latest_tag, CURRENT_VERSION) {
        println!(
            "\x1b[1;32m✔\x1b[0m lazyTrino is already up to date (\x1b[1mv{}\x1b[0m).",
            CURRENT_VERSION
        );
        return Ok(());
    }

    println!(
        "\x1b[1;36m➜\x1b[0m New version available: \x1b[1;32m{}\x1b[0m (current: v{})",
        latest_tag, CURRENT_VERSION
    );

    let asset = match asset {
        Some(a) => a,
        None => {
            println!(
                "\x1b[1;31m✖\x1b[0m No compatible release binary found for \x1b[1m{os}-{arch}\x1b[0m."
            );
            println!("  Visit https://github.com/{REPO}/releases to download manually.");
            return Ok(());
        }
    };

    println!(
        "\x1b[1;36m➜\x1b[0m Downloading \x1b[1m{}\x1b[0m...",
        asset.name
    );

    let current_exe = env::current_exe().context("Failed to determine current executable path")?;
    let parent_dir = current_exe
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    // Use a unique temporary download file in the same directory (so rename is atomic)
    let temp_download = parent_dir.join(format!(".lazytrino_update_{}", std::process::id()));
    let temp_extracted = parent_dir.join(format!(".lazytrino_bin_{}", std::process::id()));

    // Clean up temporary files on exit
    struct CleanupGuard {
        paths: Vec<PathBuf>,
    }
    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            for p in &self.paths {
                let _ = fs::remove_file(p);
            }
        }
    }
    let _guard = CleanupGuard {
        paths: vec![temp_download.clone(), temp_extracted.clone()],
    };

    let download_res = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .context("Failed to download release binary")?;

    if !download_res.status().is_success() {
        bail!(
            "Failed to download asset: HTTP status {}",
            download_res.status()
        );
    }

    let bytes = download_res
        .bytes()
        .await
        .context("Failed to read download stream")?;

    {
        let mut f =
            File::create(&temp_download).context("Failed to create temporary file for download")?;
        f.write_all(&bytes)
            .context("Failed to write downloaded data to temporary file")?;
        f.flush()?;
    }

    println!(
        "\x1b[1;32m✔\x1b[0m Download complete ({} bytes).",
        bytes.len()
    );
    println!(
        "\x1b[1;36m➜\x1b[0m Installing update to \x1b[1m{}\x1b[0m...",
        current_exe.display()
    );

    extract_binary_from_archive(&temp_download, &temp_extracted)?;
    replace_binary(&temp_extracted, &current_exe)?;

    println!(
        "\x1b[1;32m✔\x1b[0m Successfully updated lazyTrino to \x1b[1;32m{}\x1b[0m!\n",
        latest_tag
    );
    println!("Run \x1b[1mlazyTrino\x1b[0m to start using the new version.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_version() {
        assert_eq!(parse_version("0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_version("v0.1.0"), Some((0, 1, 0)));
        assert_eq!(parse_version("V1.2.3"), Some((1, 2, 3)));
        assert_eq!(parse_version("v2.0.0-rc1"), Some((2, 0, 0)));
        assert_eq!(parse_version("v3.4"), Some((3, 4, 0)));
        assert_eq!(parse_version("invalid"), None);
    }

    #[test]
    fn test_is_newer() {
        assert!(is_newer("v0.2.0", "v0.1.0"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(is_newer("v0.1.1", "0.1.0"));
        assert!(!is_newer("v0.1.0", "v0.1.0"));
        assert!(!is_newer("v0.1.0", "v0.2.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn test_match_release_asset() {
        let assets = vec![
            ReleaseAsset {
                name: "lazyTrino-v0.2.0-x86_64-unknown-linux-gnu.tar.gz".to_string(),
                browser_download_url: "https://example.com/linux_x86_64".to_string(),
                size: Some(1024),
            },
            ReleaseAsset {
                name: "lazyTrino-v0.2.0-aarch64-apple-darwin.tar.gz".to_string(),
                browser_download_url: "https://example.com/darwin_arm64".to_string(),
                size: Some(1024),
            },
            ReleaseAsset {
                name: "lazyTrino-darwin-x86_64".to_string(),
                browser_download_url: "https://example.com/darwin_x86_64".to_string(),
                size: Some(1024),
            },
        ];

        let linux_match = match_release_asset(&assets, "linux", "x86_64");
        assert!(linux_match.is_some());
        assert_eq!(
            linux_match.unwrap().name,
            "lazyTrino-v0.2.0-x86_64-unknown-linux-gnu.tar.gz"
        );

        let darwin_arm = match_release_asset(&assets, "darwin", "aarch64");
        assert!(darwin_arm.is_some());
        assert_eq!(
            darwin_arm.unwrap().name,
            "lazyTrino-v0.2.0-aarch64-apple-darwin.tar.gz"
        );

        let darwin_x86 = match_release_asset(&assets, "darwin", "x86_64");
        assert!(darwin_x86.is_some());
        assert_eq!(darwin_x86.unwrap().name, "lazyTrino-darwin-x86_64");
    }

    #[test]
    fn test_parse_github_release_json() {
        let json = r#"{
            "tag_name": "v0.2.0",
            "name": "lazyTrino v0.2.0",
            "html_url": "https://github.com/Codesmith28/lazyTrino/releases/tag/v0.2.0",
            "assets": [
                {
                    "name": "lazyTrino-v0.2.0-x86_64-unknown-linux-gnu.tar.gz",
                    "browser_download_url": "https://github.com/Codesmith28/lazyTrino/releases/download/v0.2.0/lazyTrino-x86_64.tar.gz",
                    "size": 5242880
                }
            ]
        }"#;

        let release: GithubRelease = serde_json::from_str(json).unwrap();
        assert_eq!(release.tag_name, "v0.2.0");
        assert_eq!(release.assets.len(), 1);
        assert_eq!(
            release.assets[0].name,
            "lazyTrino-v0.2.0-x86_64-unknown-linux-gnu.tar.gz"
        );
    }

    #[test]
    fn test_parse_tag_from_redirect_url() {
        let url = "https://github.com/Codesmith28/lazyTrino/releases/tag/v0.2.0";
        let tag = url.split("/releases/tag/").nth(1).unwrap();
        assert_eq!(tag, "v0.2.0");

        let url_with_query =
            "https://github.com/Codesmith28/lazyTrino/releases/tag/v1.0.0?ref=xyz#hash";
        let tag_part = url_with_query.split("/releases/tag/").nth(1).unwrap();
        let cleaned = tag_part.split(['?', '#']).next().unwrap();
        assert_eq!(cleaned, "v1.0.0");
    }

    #[test]
    fn test_logo_banner_alignment() {
        let lines: Vec<&str> = LOGO_BANNER.lines().collect();
        assert_eq!(lines.len(), 8, "Banner must have exactly 8 lines");
        assert!(
            lines[0].starts_with("  _"),
            "Line 1 must start with 2 spaces"
        );
        assert!(
            lines[1].starts_with(" | |"),
            "Line 2 must start with 1 space"
        );
        assert!(
            lines[5].starts_with(" |_|"),
            "Line 6 must start with 1 space"
        );
        assert!(
            lines[6].starts_with("               __/ |"),
            "Line 7 must preserve leading indentation under y"
        );
        assert!(
            lines[7].starts_with("              |___/"),
            "Line 8 must preserve leading indentation under y"
        );
    }
}
