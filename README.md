# labtohub

CLI that publishes GitLab main to GitHub with a single squashed commit.

## Usage
- `labtohub -m "Message"` or run and follow prompts.
- Switches to `main`, fast-forwards from `origin/main`, and either squashes changes since last GitHub release or force-publishes if histories are unrelated/empty.
- Offers to reset local `main` to `origin/main` if they diverge.

## Requirements
- Git remotes: `origin` pointing to GitLab, `github` pointing to GitHub.

## Safety
- GitLab history is never rewritten; only `github/main` is force-pushed when publishing.
