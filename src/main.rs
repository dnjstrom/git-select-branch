extern crate core;

use std::cmp::Reverse;
use std::io::ErrorKind;
use std::sync::Arc;
use std::{env, process};

use anyhow::{anyhow, Context, Result};
use dialoguer::theme::{ColorfulTheme, SimpleTheme, Theme};
use dialoguer::{FuzzySelect, Select};
use expect_exit::Expected;
use git2::{BranchType, Commit, Reference, Repository};
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
        .or_exit_(format!("No git repository discovered at {:?}", current_dir).as_str());

    let git_config = repo
        .config()
        .with_context(|| "Could not get git config")?
        .snapshot()
        .with_context(|| "Could not create a snapshot of git config")?;
    let config = Config::from_git_config(&git_config)
        .with_context(|| "Error reading configuration from git")?;

    let current_branch = get_current_branch(&repo)?;
    let sorted_branches = get_sorted_branches(&config, &repo)?;
    let options = get_branch_options(
        sorted_branches.iter().map(|s| s.as_str()).collect(),
        current_branch.as_deref(),
    );

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
                checkout(repo, selected_branch)?;
                Ok(())
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

fn checkout(repo: Repository, branch_name: &String) -> Result<()> {
    let ref_name = format!("refs/heads/{}", branch_name);

    let branch_object = repo.revparse_single(ref_name.as_str())?;

    repo.checkout_tree(&branch_object, None)?;

    repo.set_head(ref_name.as_str())?;

    Ok(())
}

fn get_branch_options(sorted_branches: Vec<&str>, current_branch: Option<&str>) -> Vec<String> {
    let all_branches: Vec<String> = sorted_branches
        .into_iter()
        .filter(|s| match current_branch {
            Some(branch) => *s != branch,
            None => true,
        })
        .map(|s| s.to_string())
        .collect();

    let mut options = Vec::new();

    match current_branch {
        Some(branch) => options.push(branch.to_string()),
        None => options.push("<no branch>".to_string()),
    }

    options.extend(all_branches);

    options
}

fn get_current_branch(repo: &Repository) -> Result<Option<String>> {
    Ok(repo
        .head()
        .or_exit_("Can't get repo head")
        .shorthand()
        .map(|s| s.to_string()))
}

fn get_sorted_branches(config: &Config, repo: &Repository) -> Result<Vec<String>> {
    let branch_refs: Vec<Reference> = repo
        .branches(match config.show_remote_branches {
            true => None,
            false => Some(BranchType::Local),
        })?
        .filter(|r| r.is_ok())
        .map(|r| r.unwrap().0.into_reference())
        .collect();

    let mut branch_name_and_commit: Vec<(String, Commit)> = branch_refs
        .iter()
        .filter_map(|r| match r.shorthand() {
            Some(shorthand) => match r.peel_to_commit() {
                Ok(commit) => Some((shorthand.to_string(), commit)),
                Err(_) => None,
            },
            None => None,
        })
        .collect();

    branch_name_and_commit.sort_by_key(|(_, commit)| Reverse(commit.time()));

    let branches = match config.limit {
        Some(limit) => branch_name_and_commit
            .iter()
            .take(limit)
            .map(|(name, _)| name.to_string())
            .collect(),
        None => branch_name_and_commit
            .iter()
            .map(|(name, _)| name.to_string())
            .collect(),
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
    use crate::{get_branch_options, get_sorted_branches};

    #[test]
    fn test_get_sorted_branches() {
        let fixture = RepoFixture::new();
        fixture.create_branch("main", 10).unwrap();
        fixture.create_branch("second", 20).unwrap();
        fixture.create_branch("third", 30).unwrap();

        let sorted_branches = get_sorted_branches(&Config::default(), &fixture.repo);
        assert_eq!(sorted_branches.unwrap(), vec!["third", "second", "main"]);
    }

    #[test]
    fn test_get_sorted_branches_including_remote() {
        let fixture = RepoFixture::new();
        fixture.create_branch("a", 10).unwrap();
        fixture.create_branch("b", 20).unwrap();
        fixture.create_branch("c", 5).unwrap();
        fixture.create_remote_branch("origin", "d", 30).unwrap();
        let mut config = Config::default();
        config.show_remote_branches = true;
        let sorted_branches = get_sorted_branches(&config, &fixture.repo);
        assert_eq!(sorted_branches.unwrap(), vec!["origin/d", "b", "a", "c"])
    }

    #[test]
    fn test_get_branch_options() {
        let options = get_branch_options(vec!["a", "b", "c"], Some("c"));
        assert_eq!(options, vec!["c", "a", "b"])
    }
}
