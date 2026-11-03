#!/usr/bin/env bash
set -euo pipefail
if [[ $# != 1 ]]; then
    echo 'usage: bash scripts/import-fixture.sh NEW_REPOSITORY_DIRECTORY' >&2
    exit 2
fi
mkdir -- "$1"
repository=$(realpath "$1")
git init --quiet --initial-branch=main --object-format=sha1 "$repository"
git -C "$repository" config core.autocrlf false
git -C "$repository" config core.hooksPath /dev/null
export GIT_AUTHOR_NAME='Annealer fixture'
export GIT_AUTHOR_EMAIL='fixture@example.invalid'
export GIT_COMMITTER_NAME="$GIT_AUTHOR_NAME"
export GIT_COMMITTER_EMAIL="$GIT_AUTHOR_EMAIL"
for ((commit=0; commit<100; commit++)); do
    printf 'commit %d\n' "$commit" >> "$repository/history.txt"
    printf 'file %d\n' "$commit" > "$repository/file-$commit.txt"
    git -C "$repository" add --all
    export GIT_AUTHOR_DATE="$((1700000000+commit)) +0000"
    export GIT_COMMITTER_DATE="$GIT_AUTHOR_DATE"
    git -C "$repository" -c commit.gpgsign=false commit --quiet -m "fixture $commit"
done
git -C "$repository" rev-parse HEAD
