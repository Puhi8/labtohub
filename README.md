# labtohub

CLI that copies GitLab `origin/main` onto GitHub `main` using a temporary worktree so your current working tree stays untouched.

## Usage
- `labtohub -m "Message"` or run and follow prompts.
- Fetches both remotes, creates a temporary worktree from `github/main`, makes a branch named from your message (or `new`), overwrites that branch with `origin/main`, commits, merges into the staging `main` branch, and pushes to `github/main`. Cleans up the temp worktree afterward.

## Requirements
- Git remotes: `origin` pointing to GitLab, `github` pointing to GitHub.

## Safety
- Uses a merge into `github/main` (no force-push). The temporary branch/worktree is recreated each run and your original working tree is not modified.
