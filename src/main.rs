extern crate clap;
extern crate colored;
#[macro_use]
extern crate failure;
extern crate futures;
extern crate git2;
extern crate itertools;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_xml_rs;

use clap::{App, SubCommand};
use colored::*;
use failure::Error;
use futures::executor::ThreadPool;
use futures::future;
use futures::prelude::*;
use git2::{Repository, Status};
use itertools::Itertools;

use std::env;
use std::fmt;
use std::io::ErrorKind::NotFound;
use std::path::{Path, PathBuf};
use std::process::Command;

mod manifest;
use manifest::{Manifest, Project};

#[derive(Debug, Fail)]
enum RepoStatusError {
    #[fail(display = "Repo not found in current directory.")]
    RepoRootNotFound,
    #[fail(display = "Manifest does not exists at: {}", path)]
    ManifestDoesNotExists { path: String },
    #[fail(display = "Invalid utf8")]
    InvalidUtf8,
}

fn find_repo_root() -> Result<PathBuf, Error> {
    let mut path = env::current_dir()?;
    loop {
        let repo_path = path.join(".repo");
        if repo_path.exists() && repo_path.is_dir() {
            return Ok(path);
        }
        path = PathBuf::from(path.parent().ok_or(RepoStatusError::RepoRootNotFound)?);
    }
}

fn find_manifest(repo_root: &Path) -> Result<PathBuf, Error> {
    let manifest = repo_root.join(".repo/manifest.xml");
    if manifest.exists() {
        Ok(manifest)
    } else {
        Err(RepoStatusError::ManifestDoesNotExists {
            path: String::from(manifest.to_str().ok_or(RepoStatusError::InvalidUtf8)?),
        }.into())
    }
}

struct GitStatus(Status);

impl fmt::Display for GitStatus {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let st = self.0;
        let index_flag = if st.contains(Status::INDEX_NEW) {
            'A'
        } else if st.contains(Status::INDEX_MODIFIED) {
            'M'
        } else if st.contains(Status::INDEX_DELETED) {
            'D'
        } else if st.contains(Status::INDEX_RENAMED) {
            'R'
        } else {
            '-'
        };
        let worktree_flag = if st.contains(Status::WT_NEW) {
            'a'
        } else if st.contains(Status::WT_MODIFIED) {
            'm'
        } else if st.contains(Status::WT_DELETED) {
            'd'
        } else if st.contains(Status::WT_TYPECHANGE) {
            't'
        } else if st.contains(Status::WT_RENAMED) {
            'r'
        } else {
            '-'
        };
        write!(f, "{}{}", index_flag, worktree_flag)
    }
}

fn get_status(repo_root: PathBuf, project: Project) -> Result<String, Error> {
    let index_change: Status =
        Status::INDEX_NEW | Status::INDEX_MODIFIED | Status::INDEX_DELETED | Status::INDEX_RENAMED;
    let worktree_change = Status::WT_NEW | Status::WT_MODIFIED | Status::WT_DELETED
        | Status::WT_TYPECHANGE | Status::WT_RENAMED;

    let project_path = &project.path.unwrap_or(project.name);
    let repo = Repository::init(repo_root.join(&project_path))?;
    let mut options = git2::StatusOptions::new();
    options.include_ignored(false);
    let statuses = repo.statuses(Some(&mut options))?
        .iter()
        .filter_map(|status| {
            if !status.status().intersects(index_change | worktree_change) {
                return None;
            }

            let st = status.status();
            let line = format!(" {}     {}", GitStatus(st), status.path().unwrap());
            if st.intersects(index_change) && !st.contains(worktree_change) {
                Some(line.green())
            } else {
                Some(line.red())
            }
        })
        .join("\n");
    if !statuses.is_empty() {
        Ok(format!("project {}/\n{}", project_path.bold(), statuses))
    } else {
        Ok(String::new())
    }
}

fn run() -> Result<(), Error> {
    let matches = App::new("repo")
        .subcommand(SubCommand::with_name("status").help("Sets the input file to use"))
        .get_matches_safe()
        .unwrap_or_else(|e| {
            println!("{:?}", e);
            // When arguments are not parseable, forward everything to the original repo command
            // TODO: make sure if this target is also named 'repo' that we don't do anything recursive (fork bomb).
            let repo_return_code = Command::new("repo")
                .args(env::args_os().skip(1))
                .status()
                .map_err(|e| {
                    if let NotFound = e.kind() {
                        println!("`repo` was not found! Check your PATH!");
                    } else {
                        println!("Some strange error occurred :(");
                    }
                })
                .unwrap();
            ::std::process::exit(repo_return_code.code().unwrap());
        });

    if let Some(_matches) = matches.subcommand_matches("status") {
        let repo_root = find_repo_root()?;
        let manifest_path = find_manifest(&repo_root)?;

        let fut_output = future::join_all(
            Manifest::from_path(&manifest_path)?
                .projects
                .into_iter()
                .map(move |project| {
                    let repo_root = repo_root.clone();
                    future::result(get_status(repo_root.clone(), project))
                }),
        ).and_then(|outputs: Vec<String>| {
            Ok(println!(
                "{}",
                outputs
                    .into_iter()
                    .filter(|line| !line.is_empty())
                    .join("\n")
            ))
        });

        ThreadPool::new()
            .expect("Failed to create threadpool")
            .run(fut_output)
    } else {
        Ok(())
    }
}

fn main() {
    if let Err(e) = run() {
        println!("{} {}", "Error:".red(), e);
        std::process::exit(1);
    }
}
