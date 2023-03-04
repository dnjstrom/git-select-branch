use anyhow::{anyhow, Context};
use dialoguer::theme::{ColorfulTheme, Theme};
use std::convert::TryFrom;
use std::sync::Arc;
use std::usize;

#[derive(Clone)]
pub struct Config {
    pub theme: Arc<dyn Theme>,
    pub fuzzy: bool,
    pub show_remote_branches: bool,
    pub limit: Option<usize>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: Arc::new(ColorfulTheme::default()),
            fuzzy: true,
            show_remote_branches: false,
            limit: Some(20usize),
        }
    }
}

macro_rules! extract_config_value {
    ($config:ident, bool, $option_name:expr) => {
        map_git2_not_found_to_none($config.get_bool($option_name))
            .with_context(|| format!("Error parsing boolean value in {}", $option_name))?
    };

    ($config:ident, str, $option_name:expr) => {
        map_git2_not_found_to_none($config.get_str($option_name))
            .with_context(|| format!("Error parsing string value in {}", $option_name))?
    };

    ($config:ident, i64, $option_name:expr) => {
        map_git2_not_found_to_none($config.get_i64($option_name))
            .with_context(|| format!("Error parsing integer value in {}", $option_name))?
    };
}

impl Config {
    pub fn from_git_config(git_config: &git2::Config) -> anyhow::Result<Config> {
        let mut config = Config::default();
        if let Some(value) = extract_config_value!(git_config, bool, "select-branch.fuzzy") {
            config.fuzzy = value;
        }

        if let Some(value) =
            extract_config_value!(git_config, bool, "select-branch.show-remote-branches")
        {
            config.show_remote_branches = value;
        }

        if let Some(value) = extract_config_value!(git_config, str, "select-branch.theme") {
            config.theme = crate::match_theme_config(value)
                .with_context(|| "Could not parse theme configuration")?;
        }

        if let Some("none") = extract_config_value!(git_config, str, "select-branch.limit") {
            config.limit = None
        } else if let Some(limit) = extract_config_value!(git_config, i64, "select-branch.limit") {
            if limit <= 0 {
                return Err(anyhow!(
                    "\"{}\" is not a valid \"select-branch.limit\" value.\n\
                    The value must be either a positive integer, or \"none\". e.g.:\n\
                    > git config --global select-branch.limit none\n\
                    or\n\
                    > git config --global select-branch.limit 20",
                    limit
                ));
            }
            config.limit = Some(
                usize::try_from(limit)
                    .with_context(|| format!("Can't convert {limit:?} to usize"))?,
            )
        }

        Ok(config)
    }
}

fn map_git2_not_found_to_none<E>(
    config_result: anyhow::Result<E, git2::Error>,
) -> Result<Option<E>, git2::Error> {
    config_result
        .map(|value| Some(value))
        .or_else(|err| match err.code() {
            git2::ErrorCode::NotFound => Ok(None),
            _ => Err(err),
        })
}
