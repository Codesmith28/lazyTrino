use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::config::{CliArgs, ConnectionConfig};

const APP_DIR: &str = "lazytrino";
const CONFIG_FILE_NAME: &str = "config.toml";
const URL_ENV_VAR: &str = "LAZYTRINO_URL";
const USER_ENV_VAR: &str = "LAZYTRINO_USER";
const PASSWORD_ENV_VAR: &str = "LAZYTRINO_PASSWORD";

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_profile: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub profiles: BTreeMap<String, StoredConnectionConfig>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_used: Option<StoredConnectionConfig>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredConnectionConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedConnectionConfig {
    pub config: ConnectionConfig,
    pub auto_connect: bool,
    pub selected_profile: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct EnvOverrides {
    url: Option<String>,
    user: Option<String>,
    password: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum ValueSource {
    Default,
    LastUsed,
    Profile,
    Env,
    Cli,
}

impl EnvOverrides {
    fn from_env() -> Self {
        Self {
            url: std::env::var(URL_ENV_VAR).ok(),
            user: std::env::var(USER_ENV_VAR).ok(),
            password: std::env::var(PASSWORD_ENV_VAR).ok(),
        }
    }
}

pub fn config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join(APP_DIR).join(CONFIG_FILE_NAME))
}

pub fn load() -> Option<ConfigFile> {
    let path = config_path()?;
    let contents = fs::read_to_string(&path).ok()?;
    parse_config(&contents, Some(&path))
}

pub fn resolve_profile(
    args: &CliArgs,
    config_file: Option<&ConfigFile>,
) -> ResolvedConnectionConfig {
    resolve_profile_with_env(args, config_file, EnvOverrides::from_env())
}

pub fn save_last_used(url: &str, user: &str) -> Result<()> {
    let path = config_path().context("could not determine config directory")?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    }

    let mut config = load_for_save(&path)?;
    config.last_used = Some(StoredConnectionConfig {
        url: Some(url.to_string()),
        user: Some(user.to_string()),
        password: None,
    });

    let contents = toml::to_string_pretty(&config).context("failed to serialize config file")?;
    fs::write(&path, contents)
        .with_context(|| format!("failed to write config file {}", path.display()))?;

    Ok(())
}

fn load_for_save(path: &Path) -> Result<ConfigFile> {
    if !path.exists() {
        return Ok(ConfigFile::default());
    }

    let contents = fs::read_to_string(path)
        .with_context(|| format!("failed to read config file {}", path.display()))?;

    toml::from_str(&contents)
        .with_context(|| format!("failed to parse existing config file {}", path.display()))
}

fn parse_config(contents: &str, path: Option<&Path>) -> Option<ConfigFile> {
    match toml::from_str(contents) {
        Ok(config) => Some(config),
        Err(error) => {
            if let Some(path) = path {
                warn!(path = %path.display(), error = %error, "Failed to parse config file");
            } else {
                warn!(error = %error, "Failed to parse config file");
            }
            None
        }
    }
}

fn resolve_profile_with_env(
    args: &CliArgs,
    config_file: Option<&ConfigFile>,
    env: EnvOverrides,
) -> ResolvedConnectionConfig {
    let default_config = ConnectionConfig::default();
    let (selected_profile_name, selected_profile) = selected_profile(config_file, args);
    let last_used = config_file.and_then(|config| config.last_used.as_ref());

    let (url, url_source) = resolve_string(
        args.url.clone(),
        env.url,
        selected_profile.and_then(|profile| profile.url.clone()),
        last_used.and_then(|last| last.url.clone()),
        default_config.url,
    );
    let (user, user_source) = resolve_string(
        args.user.clone(),
        env.user,
        selected_profile.and_then(|profile| profile.user.clone()),
        last_used.and_then(|last| last.user.clone()),
        default_config.user,
    );
    let (password, _) = resolve_string(
        args.password.clone(),
        env.password,
        selected_profile.and_then(|profile| profile.password.clone()),
        None,
        default_config.password,
    );

    ResolvedConnectionConfig {
        config: ConnectionConfig {
            url,
            user,
            password,
        },
        auto_connect: url_source >= ValueSource::Profile && user_source >= ValueSource::Profile,
        selected_profile: selected_profile_name,
    }
}

fn selected_profile<'a>(
    config_file: Option<&'a ConfigFile>,
    args: &CliArgs,
) -> (Option<String>, Option<&'a StoredConnectionConfig>) {
    let Some(config_file) = config_file else {
        return (None, None);
    };

    let Some(profile_name) = args
        .profile
        .as_deref()
        .or(config_file.default_profile.as_deref())
    else {
        return (None, None);
    };

    match config_file.profiles.get(profile_name) {
        Some(profile) => (Some(profile_name.to_string()), Some(profile)),
        None => (None, None),
    }
}

fn resolve_string(
    cli: Option<String>,
    env: Option<String>,
    profile: Option<String>,
    last_used: Option<String>,
    default: String,
) -> (String, ValueSource) {
    if let Some(value) = cli {
        return (value, ValueSource::Cli);
    }
    if let Some(value) = env {
        return (value, ValueSource::Env);
    }
    if let Some(value) = profile {
        return (value, ValueSource::Profile);
    }
    if let Some(value) = last_used {
        return (value, ValueSource::LastUsed);
    }

    (default, ValueSource::Default)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> CliArgs {
        CliArgs::default()
    }

    fn sample_config() -> ConfigFile {
        toml::from_str(
            r#"
default_profile = "local"

[profiles.local]
url = "http://localhost:8080"
user = "trino"

[profiles.prod]
url = "https://trino.example.com:8443"
user = "admin"
password = "insecure"

[last_used]
url = "http://last-used:8080"
user = "recent"
"#,
        )
        .unwrap()
    }

    #[test]
    fn parses_sample_config_toml() {
        let config = sample_config();

        assert_eq!(config.default_profile.as_deref(), Some("local"));
        assert_eq!(
            config.profiles["local"].url.as_deref(),
            Some("http://localhost:8080")
        );
        assert_eq!(
            config.profiles["prod"].password.as_deref(),
            Some("insecure")
        );
        assert_eq!(
            config
                .last_used
                .as_ref()
                .and_then(|last| last.user.as_deref()),
            Some("recent")
        );
    }

    #[test]
    fn resolves_profile_env_and_cli_precedence() {
        let mut args = args();
        args.profile = Some("prod".to_string());
        args.user = Some("cli-user".to_string());

        let resolved = resolve_profile_with_env(
            &args,
            Some(&sample_config()),
            EnvOverrides {
                url: Some("http://env-host:8080".to_string()),
                user: Some("env-user".to_string()),
                password: Some("env-password".to_string()),
            },
        );

        assert_eq!(resolved.selected_profile.as_deref(), Some("prod"));
        assert_eq!(
            resolved.config,
            ConnectionConfig {
                url: "http://env-host:8080".to_string(),
                user: "cli-user".to_string(),
                password: "env-password".to_string(),
            }
        );
        assert!(resolved.auto_connect);
    }

    #[test]
    fn malformed_config_falls_back_gracefully() {
        let malformed = parse_config("default_profile = [", None);

        let resolved =
            resolve_profile_with_env(&args(), malformed.as_ref(), EnvOverrides::default());

        assert!(malformed.is_none());
        assert_eq!(resolved.config, ConnectionConfig::default());
        assert!(!resolved.auto_connect);
    }
}
