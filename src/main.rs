use anyhow::{bail, Result};
use dialoguer::{Confirm, Input};
use std::env::args;
use std::fs;
use std::process::{Command, Stdio};

const TMP_WORKTREE: &str = ".labtohub-tmp";
const MAIN_STAGING_BRANCH: &str = "labtohub-main";

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

fn run_git_in(path: &str, args: &[&str]) -> Result<()> {
   let mut full = vec!["-C", path];
   full.extend_from_slice(args);
   run("git", &full)
}

fn branch_name_from_message(message: &str) -> String {
   let mut name = message
      .trim()
      .chars()
      .map(|c| {
         if c.is_ascii_alphanumeric() {
            c.to_ascii_lowercase()
         } else {
            '-'
         }
      })
      .collect::<String>();
   while name.contains("--") {
      name = name.replace("--", "-");
   }
   name = name.trim_matches('-').to_string();
   if name.is_empty() {
      "new".to_string()
   } else {
      name
   }
}

fn fetch_remotes() -> Result<()> {
   println!("Fetching github/main and origin/main...");
   run("git", &["fetch", "github", "main"])?;
   run("git", &["fetch", "origin", "main"])?;
   Ok(())
}

fn remove_existing_worktree() -> Result<()> {
   let _ = run("git", &["worktree", "remove", "--force", TMP_WORKTREE]);
   let _ = fs::remove_dir_all(TMP_WORKTREE);
   Ok(())
}

fn add_base_worktree() -> Result<()> {
   println!(
      "Adding temporary worktree '{}' from github/main...",
      TMP_WORKTREE
   );
   run(
      "git",
      &[
         "worktree",
         "add",
         "--force",
         "-B",
         MAIN_STAGING_BRANCH,
         TMP_WORKTREE,
         "github/main",
      ],
   )?;
   Ok(())
}

fn create_content_branch(branch: &str) -> Result<()> {
   println!("Creating branch '{}' in worktree...", branch);
   run_git_in(TMP_WORKTREE, &["switch", "-C", branch])?;
   Ok(())
}

fn overwrite_with_origin_main() -> Result<()> {
   println!("Overwriting worktree with origin/main contents...");
   run_git_in(
      TMP_WORKTREE,
      &["restore", "--source", "origin/main", "--staged", "--worktree", "."],
   )?;
   run_git_in(TMP_WORKTREE, &["clean", "-fd"])?;
   Ok(())
}

fn commit_worktree(message: &str) -> Result<bool> {
   run_git_in(TMP_WORKTREE, &["add", "-A"])?;
   let status = Command::new("git")
      .args(&["-C", TMP_WORKTREE, "diff", "--cached", "--quiet"])
      .status()?;
   if status.success() {
      println!("No differences between github/main and origin/main; nothing to commit.");
      return Ok(false);
   }
   run_git_in(TMP_WORKTREE, &["commit", "-m", message])?;
   Ok(true)
}

fn merge_into_main(branch: &str, message: &str) -> Result<()> {
   println!("Merging '{}' into staging main branch...", branch);
   run_git_in(TMP_WORKTREE, &["switch", MAIN_STAGING_BRANCH])?;
   run_git_in(TMP_WORKTREE, &["merge", "--no-ff", branch, "-m", message])?;
   Ok(())
}

fn push_to_github_main() -> Result<()> {
   println!("Pushing merged main to github/main...");
   let target = format!("{}:main", MAIN_STAGING_BRANCH);
   run_git_in(TMP_WORKTREE, &["push", "github", &target])?;
   Ok(())
}

struct Cleanup {
   worktree_created: bool,
}

impl Cleanup {
   fn new() -> Self {
      Cleanup {
         worktree_created: false,
      }
   }

   fn mark_worktree(&mut self) {
      self.worktree_created = true;
   }
}

impl Drop for Cleanup {
   fn drop(&mut self) {
      if self.worktree_created {
         let _ = Command::new("git")
            .args(&["worktree", "remove", "--force", TMP_WORKTREE])
            .status();
         let _ = fs::remove_dir_all(TMP_WORKTREE);
      }
   }
}

fn main() -> Result<()> {
   let mut cleanup = Cleanup::new();

   let mut argv = args().skip(1).collect::<Vec<_>>();
   let mut message = String::new();
   if argv.len() >= 2 && argv[0] == "-m" {
      message = argv[1].clone();
      argv.drain(0..2);
   }
   if message.is_empty() {
      message = Input::new()
         .with_prompt("Enter merge message")
         .interact_text()?;
   }
   let branch = branch_name_from_message(&message);

   println!("Branch to create: '{}'", branch);
   println!("Merge message: \"{}\"", message);
   if !Confirm::new()
      .with_prompt("Proceed? Uses a temporary worktree; your current files stay untouched.")
      .default(false)
      .interact()?
   {
      bail!("Aborted");
   }

   fetch_remotes()?;
   remove_existing_worktree()?;
   add_base_worktree()?;
   cleanup.mark_worktree();

   create_content_branch(&branch)?;
   overwrite_with_origin_main()?;

   if !commit_worktree(&message)? {
      println!("Done. No changes to publish.");
      return Ok(());
   }

   merge_into_main(&branch, &message)?;
   push_to_github_main()?;

   println!(
      "Done: origin/main copied onto github/main via branch '{}' (worktree cleaned).",
      branch
   );
   Ok(())
}
