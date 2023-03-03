use anyhow::Result;
use git2::{Repository, RepositoryInitOptions, Signature, Time};

use crate::config::Config;
use tempfile::TempDir;

pub fn repo_init() -> (TempDir, Repository) {
    let td = TempDir::new().unwrap();
    let mut opts = RepositoryInitOptions::new();
    opts.initial_head("main");
    let repo = Repository::init_opts(td.path(), &opts).unwrap();
    {
        let mut config = repo.config().unwrap();
        config.set_str("user.name", "name").unwrap();
        config.set_str("user.email", "email").unwrap();
    }
    (td, repo)
}

#[derive()]
pub struct RepoFixture {
    tempdir: TempDir,
    repo: Repository,
}

impl RepoFixture {
    pub fn new() -> Self {
        let (tempdir, repo) = repo_init();
        Self { tempdir, repo }
    }

    pub fn tempdir(&self) -> &TempDir {
        &self.tempdir
    }

    pub fn repo(&self) -> &Repository {
        &self.repo
    }

    pub fn config(&self) -> Result<Config> {
        Ok(Config::from_git_config(&self.repo.config()?.snapshot()?)?)
    }

    pub fn create_branch(&self, name: &str, commit_time_seconds: i64) -> Result<()> {
        let time = Time::new(commit_time_seconds, 0);
        let default_signature = self.repo.signature()?;
        let signature = Signature::new(
            default_signature.name().unwrap(),
            default_signature.email().unwrap(),
            &time,
        )?;
        let tree_id = self.repo.index()?.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        let _ = self.repo.commit(
            Some(format!("refs/heads/{}", name).as_str()),
            &signature,
            &signature,
            format!("commit at {:?}", time).as_str(),
            &tree,
            &[],
        )?;
        Ok(())
    }

    pub fn create_remote_branch(
        &self,
        remote_name: &str,
        name: &str,
        commit_time_seconds: i64,
    ) -> Result<()> {
        let time = Time::new(commit_time_seconds, 0);
        let default_signature = self.repo.signature()?;
        let signature = Signature::new(
            default_signature.name().unwrap(),
            default_signature.email().unwrap(),
            &time,
        )?;
        let tree_id = self.repo.index()?.write_tree()?;
        let tree = self.repo.find_tree(tree_id)?;

        let _ = self.repo.commit(
            Some(format!("refs/remotes/{}/{}", remote_name, name).as_str()),
            &signature,
            &signature,
            format!("commit at {:?}", time).as_str(),
            &tree,
            &[],
        )?;
        Ok(())
    }
}
