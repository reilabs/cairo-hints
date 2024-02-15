use std::path::PathBuf;

use scarb_metadata::PackageMetadata;

pub fn absolute_path(
    package: &PackageMetadata,
    arg: Option<PathBuf>,
    config_key: &str,
    default: Option<PathBuf>,
) -> Option<PathBuf> {
    let manifest_path = package.manifest_path.clone().into_std_path_buf();
    let project_dir = manifest_path.parent().expect(
        format!(
            "Invalid manifest path {}",
            package.manifest_path.clone().into_string()
        )
        .as_str(),
    );

    let definitions = arg
        .or_else(|| {
            package
                .tool_metadata("hints")
                .and_then(|tool_config| tool_config[config_key].as_str().map(PathBuf::from))
        })
        .or(default)?;

    if definitions.is_absolute() {
        Some(definitions)
    } else {
        Some(project_dir.join(definitions))
    }
}
