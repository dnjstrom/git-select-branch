use std::env;

use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, Select};
use expect_exit::Expected;
use git2::{BranchType, Commit, Reference, Repository};

/// Tiny CLI utility to checkout a recent git branch interactively.
fn main() -> Result<()> {
    let current_dir = env::current_dir().or_exit_("Could not get current directory");
    let repo = Repository::discover(current_dir.as_path())
        .or_exit_(format!("No git repository discovered at {:?}", current_dir).as_str());

    let current_branch_owned = get_current_branch(&repo)?;
    let current_branch = current_branch_owned;
    let sorted_branches = get_sorted_branches(&repo)?;
    let options = get_branch_options(
        sorted_branches.iter().map(|s| s.as_str()).collect(),
        current_branch.as_deref(),
    );

    let result = Select::with_theme(&ColorfulTheme::default())
        .items(&options)
        .paged(true)
        .default(0)
        .with_prompt("Which branch would you like to switch to?")
        .interact_opt()
        .expect("No selection");

    match result {
        Some(selection) => {
            let selected_branch = &options[selection];
            checkout(repo, selected_branch)?;
        }
        None => match current_branch {
            Some(branch) => println!("Stayed on branch '{}'", &branch),
            None => println!("Doing nothing"),
        },
    }

    Ok(())
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

fn get_sorted_branches(repo: &Repository) -> Result<Vec<String>> {
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

    let branches = branch_name_and_commit
        .iter()
        .take(20)
        .map(|(name, _)| name.to_string())
        .collect();

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
