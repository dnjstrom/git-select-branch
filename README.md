# git-select-branch

Tiny Rust CLI to checkout a recent git branch interactively.

![git-select-branch lets you select a recent branch interactively.](./screenshot.gif)


## Installation

### Homebrew

```bash
brew tap dnjstrom/git-select-branch
brew install git-select-branch
```

### Cargo

```bash
cargo install git-select-branch
```

### Sources

```bash
git clone git@github.com:dnjstrom/git-select-branch.git
cd git-select-branch
cargo install --path .
```

## Git alias

Add the following section to your `~/.gitconfig`:

```toml
[alias]
  select-branch = "!git-select-branch"
```

Now you can simply type `git select-branch` to switch between branches.
