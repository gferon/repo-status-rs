use serde_xml_rs::deserialize;

use failure::Error;

use std::fs::File;
use std::io::BufReader;
use std::path::Path;

#[derive(Deserialize, Debug)]
pub struct Manifest {
    #[serde(rename = "remote", default)]
    pub remotes: Vec<Remote>,
    #[serde(rename = "default", default)]
    pub defaults: Vec<Default>,
    #[serde(rename = "project", default)]
    pub projects: Vec<Project>,
}

#[derive(Debug, Deserialize)]
pub struct Default {
    pub revision: String,
    pub remote: String,
}

#[derive(Debug, Deserialize)]
pub struct Remote {
    pub name: String,
    pub fetch: String,
    pub review: String,
}

#[derive(Debug, Deserialize)]
pub struct Project {
    pub name: String,
    pub path: Option<String>,
    pub groups: Option<String>,
    pub revision: Option<String>,
}

impl Manifest {
    pub fn from_path(path: &Path) -> Result<Manifest, Error> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let manifest: Manifest = deserialize(reader)?;
        Ok(manifest)
    }
}