# git-select-branch

Tiny rust cli to make it easier to navigate between branches by letting you interactively select
one to check out. Sorts branches aside from the current one by most recent commit.

![git-select-branch lets you select a recent branch interactively.](./screenshot.gif)


## Installation

The package is currently only available as sources. 

```bash
git clone git@github.com:dnjstrom/git-select-branch.git
cd git-select-branch
cargo build --release
cargo install --path .
```

## Git alias

Add the following section to your `~/.gitconfig`:

```toml
[alias]
  select-branch = "!git-select-branch"
```

Now you can simply type `git select-branch` to select between branches.