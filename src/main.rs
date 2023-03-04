extern crate core;

use std::cell::Ref;
use std::cmp::Reverse;
use std::fmt::{Display, Formatter, Pointer};
use std::io::ErrorKind;
use std::sync::Arc;
use std::{env, process};

use anyhow::{anyhow, Context, Result};
use dialoguer::theme::{ColorfulTheme, SimpleTheme, Theme};
use dialoguer::{FuzzySelect, Select};
use expect_exit::Expected;
use git2::{BranchType, Commit, Reference, Repository, Signature, Time};
use thiserror::Error;

use config::Config;

/// Tiny CLI utility to checkout a recent git branch interactively.
fn main() -> Result<()> {
    match run_tui() {
        Ok(()) => Ok(()),
        Err(e) => match e.downcast_ref::<SelectBranchError>() {
            Some(SelectBranchError::Aborted) => process::exit(1),
            Some(SelectBranchError::Interrupted) => process::exit(2),
            None => Err(e),
        },
    }
}

#[derive(Error, Debug)]
pub enum SelectBranchError {
    #[error("Interaction aborted")]
    Aborted,
    #[error("Interaction interrupted")]
    Interrupted,
}

fn run_tui() -> Result<()> {
    let current_dir = env::current_dir().or_exit_("Could not get current directory");
    let repo = Repository::discover(current_dir.as_path())
        .or_exit_(format!("No git repository discovered at {current_dir:?}").as_str());

    let git_config = repo
        .config()
        .with_context(|| "Could not get git config")?
        .snapshot()
        .with_context(|| "Could not create a snapshot of git config")?;

    let config = Config::from_git_config(&git_config)
        .with_context(|| "Error reading configuration from git")?;

    let current_branch = get_current_branch(&repo)?;
    let sorted_choices = get_sorted_choices(&config, &repo)?;
    let options = get_branch_options(sorted_choices.clone(), current_branch.as_deref());

    ctrlc::set_handler(move || {
        dialoguer_reset_cursor_hack();
    })?;

    let prompt_result = match config.fuzzy {
        true => FuzzySelect::with_theme(config.theme.as_ref())
            .items(&options)
            .default(0)
            .with_prompt("Which branch would you like to switch to?")
            .interact_opt()
            .with_context(|| "Prompt interrupted"),
        false => Select::with_theme(config.theme.as_ref())
            .items(&options)
            .default(0)
            .with_prompt("Which branch would you like to switch to?")
            .interact_opt()
            .with_context(|| "Prompt interrupted"),
    };

    match prompt_result {
        Ok(option) => match option {
            Some(selection) => {
                let selected_branch = &options[selection];
                match selected_branch {
                    Choice::Default(_) => Err(SelectBranchError::Aborted.into()),
                    Choice::Branch(branch_info) => checkout(repo, branch_info),
                }
            }
            None => Err(SelectBranchError::Aborted.into()),
        },
        // If err, figure out if it was a SIGINT and ...
        Err(err) => match err.downcast_ref::<std::io::Error>() {
            Some(io_err) => match io_err.kind() {
                // ... if so replace err with a shorter version.
                ErrorKind::Interrupted => Err(SelectBranchError::Interrupted.into()),
                _ => Err(err),
            },
            None => Err(err),
        },
    }
}

/// `dialoguer` doesn't clean up your term if it's aborted via e.g. `SIGINT` or other exceptions:
/// https://github.com/console-rs/dialoguer/issues/188.
///
/// `dialoguer`, as a library, doesn't want to mess with signal handlers,
/// but we, as an application, are free to mess with signal handlers if we feel like it, since we
/// own the process.
fn dialoguer_reset_cursor_hack() {
    let term = dialoguer::console::Term::stdout();
    let _ = term.show_cursor();
}

fn match_theme_config(theme_name: &str) -> Result<Arc<dyn Theme>> {
    match theme_name {
        "colorful" => Ok(Arc::new(ColorfulTheme::default())),
        "simple" => Ok(Arc::new(SimpleTheme)),
        value => Err(anyhow!(
            "{} is not a valid theme, expected one of \"colorful\", \"simple\"",
            value
        )),
    }
}

#[derive(Debug, Clone)]
struct BranchInfo {
    pub shorthand: String,
    pub branch_type: BranchType,
    pub commit_time: Time,
    pub commit_message: Option<String>,
    pub commit_author_name: Option<String>,
}

#[derive(Debug, Clone)]
enum Choice {
    Default(String),
    Branch(BranchInfo),
}

impl Display for Choice {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Choice::Default(s) => write!(f, "{}", s),
            Choice::Branch(branch_info) => {
                write!(f, "{}", branch_info.shorthand,)
            }
        }
    }
}

impl From<BranchInfo> for Choice {
    fn from(value: BranchInfo) -> Self {
        Choice::Branch(value)
    }
}

fn checkout(repo: Repository, branch_info: &BranchInfo) -> Result<()> {
    let shorthand = branch_info.shorthand.as_str();
    let ref_name = match branch_info.branch_type {
        BranchType::Local => format!("refs/heads/{shorthand}"),
        BranchType::Remote => format!("refs/remotes/{shorthand}"),
    };

    let branch_object = repo.revparse_single(ref_name.as_str())?;

    repo.checkout_tree(&branch_object, None)?;

    repo.set_head(ref_name.as_str())?;

    Ok(())
}

fn get_branch_options(
    sorted_branches: Vec<BranchInfo>,
    current_branch: Option<&str>,
) -> Vec<Choice> {
    let mut branches = sorted_branches;
    if let Some(branch) = current_branch {
        branches = branches
            .iter()
            .filter(|c| c.shorthand != branch)
            .map(Clone::clone)
            .collect();
    }

    let mut options = Vec::new();

    options.push(Choice::Default(match current_branch {
        Some(branch) => branch.to_string(),
        None => "<no branch>".to_string(),
    }));

    options.extend(branches.iter().map(|b| Choice::Branch(b.clone())));

    options
}

fn get_current_branch(repo: &Repository) -> Result<Option<String>> {
    Ok(repo
        .head()
        .or_exit_("Can't get repo head")
        .shorthand()
        .map(|s| s.to_string()))
}

fn get_choices(config: &Config, repo: &Repository) -> Result<Vec<BranchInfo>> {
    Ok(repo
        .branches(match config.show_remote_branches {
            true => None,
            false => Some(BranchType::Local),
        })?
        .filter_map(|r| match r {
            Ok((branch, branch_type)) => {
                let reference = branch.into_reference();
                match reference.shorthand() {
                    Some(shorthand) => match reference.peel_to_commit() {
                        Ok(commit) => Some(BranchInfo {
                            shorthand: shorthand.to_string(),
                            branch_type: branch_type.clone(),
                            commit_message: commit.message().map(|s| s.to_string()),
                            commit_author_name: commit.author().name().map(ToString::to_string),
                            commit_time: commit.time(),
                        }),
                        Err(_) => None,
                    },
                    None => None,
                }
            }
            Err(_) => None,
        })
        .collect())
}

fn get_sorted_choices(config: &Config, repo: &Repository) -> Result<Vec<BranchInfo>> {
    let mut choices = get_choices(config, repo)?;

    choices.sort_by_key(|choice| Reverse(choice.commit_time));

    let branches = match config.limit {
        Some(limit) => choices.iter().take(limit).map(|c| c.clone()).collect(),
        None => choices,
    };
    Ok(branches)
}

#[cfg(test)]
#[macro_use]
mod test;
mod config;

#[cfg(test)]
mod tests {
    use crate::config::Config;
    use crate::test::RepoFixture;
    use crate::{get_branch_options, get_sorted_choices};

    #[test]
    fn test_get_sorted_branches_default_config() {
        let fixture = RepoFixture::new();
        fixture.create_branch("main", 10).unwrap();
        fixture.create_branch("second", 20).unwrap();
        fixture.create_branch("third", 30).unwrap();

        let sorted_branches = get_sorted_choices(&Default::default(), &fixture.repo);
        assert_eq!(sorted_branches.unwrap(), vec!["third", "second", "main"]);
    }

    #[test]
    fn test_get_sorted_branches_including_remote() {
        let fixture = RepoFixture::new();
        fixture.create_branch("a", 10).unwrap();
        fixture.create_branch("b", 20).unwrap();
        fixture.create_branch("c", 5).unwrap();
        fixture.create_remote_branch("origin", "d", 30).unwrap();
        let config = Config {
            show_remote_branches: true,
            ..Default::default()
        };
        let sorted_branches = get_sorted_choices(&config, &fixture.repo);
        assert_eq!(sorted_branches.unwrap(), vec!["origin/d", "b", "a", "c"])
    }

    #[test]
    fn test_get_sorted_branches_limit() {
        let fixture = RepoFixture::new();
        fixture.create_branch("a", 1).unwrap();
        fixture.create_branch("b", 2).unwrap();
        fixture.create_branch("c", 3).unwrap();
        let config = Config {
            limit: Some(2),
            ..Default::default()
        };
        let sorted_branches = get_sorted_choices(&config, &fixture.repo).unwrap();
        assert_eq!(sorted_branches, vec!["c", "b"])
    }

    #[test]
    fn test_get_sorted_branches_unlimited() {
        let fixture = RepoFixture::new();
        let mut expected_sorted_branches = vec![];
        for i in (0..100).rev() {
            let branch_name = format!("a-{}", i);
            expected_sorted_branches.push(branch_name.clone());
            fixture.create_branch(branch_name.as_str(), i).unwrap();
        }
        let config = Config {
            limit: None,
            ..Default::default()
        };
        let sorted_branches = get_sorted_choices(&config, &fixture.repo).unwrap();
        assert_eq!(sorted_branches.len(), 100);
        assert_eq!(sorted_branches, expected_sorted_branches)
    }

    #[test]
    fn test_get_branch_options() {
        let options = get_branch_options(vec!["a", "b", "c"], Some("c"));
        assert_eq!(options, vec!["c", "a", "b"])
    }
}
