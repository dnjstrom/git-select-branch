extern crate core;

use std::convert::TryFrom;
use std::io::ErrorKind;
use std::{env, usize};

use anyhow::{anyhow, Context, Result};
use dialoguer::theme::{ColorfulTheme, SimpleTheme, Theme};
use dialoguer::{FuzzySelect, Select};
use expect_exit::Expected;
use git2::{BranchType, Commit, Reference, Repository};

/// Tiny CLI utility to checkout a recent git branch interactively.
fn main() -> Result<()> {
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
            None => Err(anyhow!("Aborted.")),
        },
        // If err, figure out if it was a SIGINT and ...
        Err(err) => match err.downcast_ref::<std::io::Error>() {
            Some(io_err) => match io_err.kind() {
                // ... if so replace err with a shorter version.
                ErrorKind::Interrupted => Err(anyhow!("Interrupted")),
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

fn match_theme_config(theme_name: &str) -> Result<Box<dyn Theme>> {
    match theme_name {
        "colorful" => Ok(Box::new(ColorfulTheme::default())),
        "simple" => Ok(Box::new(SimpleTheme)),
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
        .branches(Some(BranchType::Local))?
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

    branch_name_and_commit.sort_by(|(_, a), (_, b)| a.time().partial_cmp(&b.time()).unwrap());
    branch_name_and_commit.reverse();

    let iter = branch_name_and_commit.iter();
    let branches = match config.limit {
        Some(limit) => iter.take(limit).map(|(name, _)| name.to_string()).collect(),
        None => iter.map(|(name, _)| name.to_string()).collect(),
    };
    Ok(branches)
}

#[cfg(test)]
#[macro_use]
mod test;

#[cfg(test)]
mod tests {
    use std::fs::File;
    use std::io::prelude::*;
    use std::path::Path;

    use git2::Repository;

    use tempfile::TempDir;

    use crate::{get_branch_options, get_sorted_branches};

    fn setup_repo(td: &TempDir, repo: &Repository) {
        let mut index = repo.index().unwrap();
        File::create(&td.path().join("foo"))
            .unwrap()
            .write_all(b"foo")
            .unwrap();
        index.add_path(Path::new("foo")).unwrap();
        let id = index.write_tree().unwrap();
        let sig = repo.signature().unwrap();
        let tree = repo.find_tree(id).unwrap();
        let parent = repo
            .find_commit(repo.head().unwrap().target().unwrap())
            .unwrap();
        let second_branch = repo.branch("second", &parent, false).unwrap();
        assert!(second_branch.name().unwrap().is_some());
        repo.commit(
            second_branch.into_reference().name(),
            &sig,
            &sig,
            "second\n\nbody",
            &tree,
            &[&parent],
        )
        .unwrap();
        let third_branch = repo.branch("third", &parent, false).unwrap();
        assert!(third_branch.name().unwrap().is_some());
        let _ = repo
            .commit(
                third_branch.into_reference().name(),
                &sig,
                &sig,
                "third\n\nbody",
                &tree,
                &[&parent],
            )
            .unwrap();
    }

    #[test]
    fn test_get_sorted_branches() {
        let (td, repo) = crate::test::repo_init();
        setup_repo(&td, &repo);
        let sorted_branches = get_sorted_branches(&repo);
        assert_eq!(sorted_branches.unwrap(), vec!["third", "second", "main"]);
    }

    #[test]
    fn test_get_branch_options() {
        let (td, repo) = crate::test::repo_init();
        setup_repo(&td, &repo);

        let options = get_branch_options(vec!["a", "b", "c"], Some("c"));
        assert_eq!(options, vec!["c", "a", "b"])
    }
}

struct Config {
    theme: Box<dyn Theme>,
    fuzzy: bool,
    limit: Option<usize>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: Box::new(ColorfulTheme::default()),
            fuzzy: false,
            limit: Some(20usize),
        }
    }
}

impl Config {
    fn from_git_config(git_config: &git2::Config) -> Result<Config> {
        let mut config = Config::default();
        if let Some(value) = map_git2_not_found_to_none(git_config.get_bool("select-branch.fuzzy"))
            .with_context(|| "Error parsing select-branch.fuzzy")?
        {
            config.fuzzy = value;
        }

        if let Some(value) = map_git2_not_found_to_none(git_config.get_str("select-branch.theme"))
            .with_context(|| "Error parsing select-branch.theme")?
        {
            config.theme =
                match_theme_config(value).with_context(|| "Could not parse theme configuration")?;
        }

        if let Some("none") = map_git2_not_found_to_none(git_config.get_str("select-branch.limit"))
            .with_context(|| "Error parsing select-branch.limit")?
        {
            config.limit = None
        } else if let Some(limit) =
            map_git2_not_found_to_none(git_config.get_i64("select-branch.limit"))
                .with_context(|| "Error parsing select-branch.limit")?
        {
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
                    .with_context(|| format!("Can't convert {:?} to usize", limit))?,
            )
        }

        Ok(config)
    }
}

fn map_git2_not_found_to_none<E>(
    config_result: Result<E, git2::Error>,
) -> core::result::Result<Option<E>, git2::Error> {
    config_result
        .map(|value| Some(value))
        .or_else(|err| match err.code() {
            git2::ErrorCode::NotFound => Ok(None),
            _ => Err(err),
        })
}
