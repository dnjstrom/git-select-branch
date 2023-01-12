extern crate core;

use std::env;

use anyhow::{Context, Result};
use dialoguer::{theme::ColorfulTheme, Select};
use git2::build::CheckoutBuilder;
use git2::{BranchType, Commit, Reference, Repository};

/// Tiny CLI utility to checkout a recent git branch interactively.
fn main() -> Result<()> {
    let current_dir =
        env::current_dir().with_context(|| "Could not get current directory".to_string())?;
    let repo = Repository::discover(current_dir.as_path())
        .with_context(|| format!("No git repository discovered in {:?}", current_dir.clone()))?;

    let current_branch_owned = get_current_branch(&repo)?;
    let current_branch = current_branch_owned;
    let sorted_branches = get_sorted_branches(&repo)?;
    let options = get_branch_options(sorted_branches, current_branch.as_deref());

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
    repo.set_head(&ref_name)
        .with_context(|| format!("Could not set head to {}", ref_name))?;
    repo.checkout_head(Some(&mut CheckoutBuilder::default()))
        .with_context(|| format!("Unable to check out branch {}", branch_name))?;
    Ok(())
}

fn get_branch_options(sorted_branches: Vec<String>, current_branch: Option<&str>) -> Vec<String> {
    let all_branches: Vec<String> = sorted_branches
        .into_iter()
        .filter(|s| match current_branch {
            Some(branch) => *s != branch,
            None => true,
        })
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
        .with_context(|| "Can't get repo head".to_string())?
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
        .filter(|r| r.shorthand().is_some() && r.peel_to_commit().is_ok())
        .map(|r| {
            (
                r.shorthand().unwrap().to_string(),
                r.peel_to_commit().unwrap(),
            )
        })
        .collect();
    branch_name_and_commit.sort_by(|(_, a), (_, b)| a.time().partial_cmp(&b.time()).unwrap());
    Ok(branch_name_and_commit
        .iter()
        .take(20)
        .map(|(name, _)| name.to_string())
        .collect())
}
