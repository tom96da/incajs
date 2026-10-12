#!/bin/sh

# Copyright (c) 2026 tom96da
# SPDX-License-Identifier: MIT OR Apache-2.0

set -eu

elevate() {
    if [ "$(id -u)" -eq 0 ]; then
        echo ""
    elif command -v sudo >/dev/null 2>&1; then
        echo "sudo"
    else
        echo "Error: \`sudo\` is required but not installed." >&2
        return 1
    fi
}

change_ownership() {
    path="${1:-}"
    if [ -z "$path" ]; then
        echo "Usage: change_ownership <path>"
        return 1
    fi

    sudo=$(elevate) || return 1
    owner="$(id -un):$(id -gn)"

    if [ -d "$path" ]; then
        echo "Ensuring directory ownership: $path"
        mkdir -p "$path"
        $sudo chown -R "$owner" "$path"
        return 0
    fi

    parent_dir=$(dirname "$path")
    if [ ! -d "$parent_dir" ]; then
        echo "Directory $parent_dir does not exist. Creating it..."
        mkdir -p "$parent_dir"
    fi

    if [ -e "$path" ]; then
        echo "Ensuring file ownership: $path"
        $sudo chown "$owner" "$path"
    else
        echo "Path $path does not exist. Creating a new file and setting ownership..."
        : > "$path"
        $sudo chown "$owner" "$path"
    fi
}

main() {
    echo "Activating devcontainer..."

    # Ensure ownership of important directories and files
    change_ownership "$(npm prefix -g)"
    change_ownership "$HOME/.claude/"
    change_ownership "$HOME/.claude.json"
    change_ownership "$CARGO_HOME"
    change_ownership "$WORKSPACE_FOLDER/target/"
    change_ownership "$WORKSPACE_FOLDER/node_modules/"

    # Install dependencies
    cargo fetch
    pn i -r

    pnpm completion bash > ~/completion-for-pnpm.bash
    echo 'source ~/completion-for-pnpm.bash' >> ~/.bashrc
}

main
