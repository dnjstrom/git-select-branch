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

## Configuration

### 

## Git alias

Add the following section to your `~/.gitconfig`:

```toml
[alias]
  select-branch = "!git-select-branch"
```

Now you can simply type `git select-branch` to switch between branches.


## Publishing

1. Bump the version in `Cargo.toml` and commit.
2. Publish to crates.io by running `cargo publish`.
3. Add a tag with the same version as above and push it to automatically create a [release](https://github.com/dnjstrom/git-select-branch/releases).
4. When the [release action](https://github.com/dnjstrom/git-select-branch/actions) has finished, update the version, urls and shasums in the [hombrew tap](https://github.com/dnjstrom/homebrew-git-select-branch/edit/master/Formula/git-select-branch.rb). 
