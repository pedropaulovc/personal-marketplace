#!/bin/bash
set -e

RESET_ALL=false
FOLDER_NAME=""

for arg in "$@"; do
    if [[ "$arg" == "--all" ]]; then
        RESET_ALL=true
    else
        FOLDER_NAME="$arg"
    fi
done

CURRENT_WORKTREE=$(pwd)
FOLDER_NAME="${FOLDER_NAME:-$(basename "$CURRENT_WORKTREE")}"

# Fetch and prune remote tracking branches
git fetch --prune

# Delete stale local branches (whose remote is gone), excluding worktree branches
git branch -vv \
    | { grep ': gone]' || [[ $? -eq 1 ]]; } \
    | grep -v 'C:/src/codjiflo' \
    | awk '{print $1}' \
    | { xargs -r git branch -D 2>/dev/null || true; }

# Reset current worktree to origin/main
git checkout main
git reset --hard origin/main
git checkout "$FOLDER_NAME"
git reset --hard origin/main
git branch --unset-upstream 2>/dev/null || true

# Nuke Neon dev/e2e branches for this worktree
LETTER=$(basename "$CURRENT_WORKTREE" | grep -oP '(?:^|-)([a-zA-Z])$' | tail -c2 | tr '[:lower:]' '[:upper:]')
if [[ -n "$LETTER" ]]; then
    echo "Nuking Neon branches for worktree $LETTER..."
    node scripts/neon-branch.js nuke "$LETTER" 2>&1 || true
fi

# Install dependencies in current worktree
npm install

if [[ "$RESET_ALL" != "true" ]]; then
    echo ""
    echo "=== Current worktree updated ==="
    exit 0
fi

# Rebase and npm install in all other worktrees
git worktree list --porcelain | grep '^worktree ' | cut -d' ' -f2- | while read -r worktree_path; do
    # Skip the main repo and current worktree
    if [[ "$worktree_path" == "$CURRENT_WORKTREE" ]]; then
        continue
    fi

    # Skip if it's the main repository (not a linked worktree)
    if [[ ! -f "$worktree_path/.git" ]]; then
        continue
    fi

    echo ""
    echo ""
    echo "=== Updating worktree: $worktree_path ==="

    # Nuke Neon branches for this worktree
    WT_LETTER=$(basename "$worktree_path" | grep -oP '(?:^|-)([a-zA-Z])$' | tail -c2 | tr '[:lower:]' '[:upper:]')
    if [[ -n "$WT_LETTER" ]]; then
        echo "Nuking Neon branches for worktree $WT_LETTER..."
        node scripts/neon-branch.js nuke "$WT_LETTER" 2>&1 || true
    fi

    # Rebase the worktree branch onto origin/main
    (
        cd "$worktree_path"
        current_branch=$(git branch --show-current)
        echo "Rebasing $current_branch onto origin/main..."
        git rebase origin/main || {
            echo "Rebase failed or had conflicts, aborting..."
            git rebase --abort 2>/dev/null || true
        }
        echo "Running npm install..."
        npm install
    )
done

echo ""
echo "=== All worktrees updated ==="
