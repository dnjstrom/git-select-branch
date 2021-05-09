#!/bin/bash

# Prepares the necessary files to publish to Github Releases and Homebrew
# See: https://federicoterzi.com/blog/how-to-publish-your-rust-project-on-homebrew/

# Ensure the release build is up-to-date
cargo build --release

cd target/release

# Remove potentially stale release artefacts
rm -f git-select-branch-mac.tar.gz git-select-branch-mac.tar.gz.shasum

# Homebrew expects a tar file
tar -czf git-select-branch-mac.tar.gz git-select-branch

# The shasum of the file is also needed
shasum -a 256 git-select-branch-mac.tar.gz > git-select-branch-mac.tar.gz.shasum
