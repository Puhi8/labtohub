use anyhow::{bail, Result};
use dialoguer::{Confirm, Input};
use std::env::args;
use std::process::{Command, Stdio};

fn run(cmd: &str, args: &[&str]) -> Result<()> {
   let status = Command::new(cmd)
      .args(args)
      .stdin(Stdio::inherit())
      .stdout(Stdio::inherit())
      .stderr(Stdio::inherit())
      .status()?;
   if !status.success() {
      bail!("Command failed: {} {:?}", cmd, args);
   }
   Ok(())
}

fn run_output(cmd: &str, args: &[&str]) -> Result<String> {
   let output = Command::new(cmd).args(args).output()?;
   if !output.status.success() {
      bail!("Command failed: {} {:?}", cmd, args);
   }
   Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn parse_ahead_behind(s: &str) -> (u64, u64) {
   let parts = s.split_whitespace().collect::<Vec<_>>();
   if parts.len() >= 2 {
      let ahead = parts[0].parse().unwrap_or(0);
      let behind = parts[1].parse().unwrap_or(0);
      return (ahead, behind);
   }
   (0, 0)
}

fn merge_base(left: &str, right: &str) -> Result<Option<String>> {
   let output = Command::new("git")
      .args(&["merge-base", left, right])
      .output()?;
   if output.status.success() {
      return Ok(Some(
         String::from_utf8_lossy(&output.stdout).trim().to_string(),
      ));
   }
   let stdout = String::from_utf8_lossy(&output.stdout);
   let stderr = String::from_utf8_lossy(&output.stderr);
   if stderr.contains("No merge base")
      || stderr.contains("no common ancestor")
      || stderr.contains("Not a valid object name")
      || stderr.contains("unknown revision")
      || stderr.contains("Needed a single revision")
      || (stderr.trim().is_empty() && stdout.trim().is_empty())
   {
      return Ok(None);
   }
   bail!(
      "Command failed: git merge-base {} {} ({})",
      left,
      right,
      stderr.trim()
   )
}

fn sync_gitlab_main() -> Result<()> {
   let status = Command::new("git")
      .args(&["pull", "origin", "main", "--ff-only"])
      .stdin(Stdio::inherit())
      .stdout(Stdio::inherit())
      .stderr(Stdio::inherit())
      .status()?;
   if status.success() {
      return Ok(());
   }
   let counts = run_output(
      "git",
      &["rev-list", "--left-right", "--count", "main...origin/main"],
   )?;
   let (ahead, behind) = parse_ahead_behind(&counts);
   println!(
      "Local main diverged from origin/main (local +{}, remote +{}).",
      ahead, behind
   );
   if !Confirm::new()
      .with_prompt("Reset local main to origin/main? Local commits will be lost.")
      .default(false)
      .interact()?
   {
      println!("Aborted.");
      return Ok(());
   }
   run("git", &["reset", "--hard", "origin/main"])?;
   Ok(())
}

fn github_branch_exists() -> Result<bool> {
   let status = Command::new("git")
      .args(&["ls-remote", "--exit-code", "github", "refs/heads/main"])
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .status()?;
   match status.code() {
      Some(0) => Ok(true),
      Some(2) => Ok(false),
      Some(code) => bail!(
         "Command failed: git ls-remote --exit-code github refs/heads/main (exit {})",
         code
      ),
      None => bail!("Command failed: git ls-remote --exit-code github refs/heads/main"),
   }
}

fn main() -> Result<()> {
   let mut argv = args().skip(1).collect::<Vec<_>>();
   let mut message = String::new();
   if argv.len() >= 2 && argv[0] == "-m" {
      message = argv[1].clone();
      argv.drain(0..2);
   }
   if message.is_empty() {
      message = Input::new()
         .with_prompt("Enter commit message")
         .interact_text()?;
   }
   println!("Commit: \"{}\"", message);
   if !Confirm::new()
      .with_prompt("Continue?")
      .default(false)
      .interact()?
   {
      println!("Aborted.");
      return Ok(());
   }
   println!("Syncing GitLab main...");
   run("git", &["switch", "main"])?;
   sync_gitlab_main()?;
   let github_exists = github_branch_exists()?;
   if !github_exists {
      println!("GitHub empty -> publish initial squashed release");
      run("git", &["add", "."])?;
      let tree = run_output("git", &["write-tree"])?;
      let commit = run_output("git", &["commit-tree", &tree, "-m", &message])?;
      run("git", &["reset", "--hard", &commit])?;
      run("git", &["push", "github", "main", "--force"])?;
      println!("Published to GitHub (initial, squashed): \"{}\".", message);
      return Ok(());
   }
   println!("Fetching github/main...");
   run("git", &["fetch", "github"])?;
   let base = match merge_base("main", "github/main")? {
      Some(base) => base,
      None => {
         println!("GitHub history unrelated to GitLab.");
         if !Confirm::new()
            .with_prompt("Force-publish current main to github/main? This overwrites GitHub.")
            .default(false)
            .interact()?
         {
            println!("Aborted.");
            return Ok(());
         }
         run("git", &["add", "."])?;
         let tree = run_output("git", &["write-tree"])?;
         let commit = run_output("git", &["commit-tree", &tree, "-m", &message])?;
         run("git", &["reset", "--hard", &commit])?;
         run("git", &["push", "github", "main", "--force"])?;
         println!("Published to GitHub (force overwrite): \"{}\".", message);
         return Ok(());
      }
   };
   println!("Base commit: {}", base);
   println!("Squashing and force-pushing...");
   run("git", &["reset", &base])?;
   run("git", &["add", "."])?;
   let result = Command::new("git")
      .args(&["commit", "-m", &message])
      .stdin(Stdio::inherit())
      .stdout(Stdio::inherit())
      .stderr(Stdio::inherit())
      .status()?;
   if !result.success() {
      println!("No changes since last GitHub release. Nothing to publish.");
      return Ok(());
   }
   run("git", &["push", "github", "main", "--force"])?;
   println!("Published to GitHub: \"{}\".", message);
   Ok(())
}
