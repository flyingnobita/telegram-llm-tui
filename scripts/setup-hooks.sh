#!/usr/bin/env bash
set -e

# Get the root of the git repo
REPO_ROOT=$(git rev-parse --show-toplevel)
HOOK_DIR="$REPO_ROOT/.git/hooks"
SOURCE_HOOK="$REPO_ROOT/scripts/git-hooks/pre-commit"

echo "Setting up git hooks in $HOOK_DIR..."

# Create hooks directory if it doesn't exist
mkdir -p "$HOOK_DIR"

# Symlink pre-commit
# We use a relative path for the symlink so it works if the repo is moved
ln -sf "../../scripts/git-hooks/pre-commit" "$HOOK_DIR/pre-commit"

# Symlink pre-push
ln -sf "../../scripts/git-hooks/pre-push" "$HOOK_DIR/pre-push"

# Ensure the source script is executable
chmod +x "$SOURCE_HOOK"
chmod +x "$REPO_ROOT/scripts/git-hooks/pre-push"

echo "Git hooks installed successfully!"
