use crate::fsx;
use anyhow::Result;
use camino::Utf8PathBuf;
use once_cell::sync::Lazy;
use scarb::core::{Config, PackageName};
use serde_json::json;
use crate::templates::get_template_engine;

pub const SERVER_MANIFEST_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["python", "requirements.txt"].iter().collect());
pub const SERVER_SOURCE_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["python/src", "main.py"].iter().collect());
pub const GITIGNORE_PATH: Lazy<Utf8PathBuf> = Lazy::new(|| [".gitignore"].iter().collect());
pub const DOCKERFILE_PATH: Lazy<Utf8PathBuf> = Lazy::new(|| ["Dockerfile"].iter().collect());
pub const PRE_COMMIT_CONFIG: Lazy<Utf8PathBuf> = Lazy::new(|| [".pre-commit-config.yaml"].iter().collect());

pub fn mk_python(canonical_path: &Utf8PathBuf, _: &PackageName, _config: &Config) -> Result<()> {
    // Get the templates registry
    let registry = get_template_engine();

    // Create the `requirements.txt` file.
    let filename = canonical_path.join(SERVER_MANIFEST_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            registry.render("requirements", &json!({}))?,
        )?;
    }

    // Create the `Dockerfile` file.
    let filename = canonical_path.join(DOCKERFILE_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            registry.render("dockerfile", &json!({}))?,
        )?;
    }

    // Create the `pre-commit` file.
    let filename = canonical_path.join(PRE_COMMIT_CONFIG.as_path());
    let pre_commit = registry.render("pre-commit", &json!({}))?;
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            pre_commit,
        )?;
    }

    // Create the `main.py` file.
    let filename = canonical_path.join(SERVER_SOURCE_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            registry.render("main", &json!({}))?
        )?;
    }

    // Create the `.gitignore` file.
    let filename = canonical_path.join(GITIGNORE_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            registry.render("gitignore", &json!({}))?
        )?;
    }

    Ok(())
}
