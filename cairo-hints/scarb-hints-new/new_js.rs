use crate::fsx;
use anyhow::Result;
use camino::Utf8PathBuf;
use indoc::{formatdoc, indoc};
use once_cell::sync::Lazy;
use scarb::core::{Config, PackageName};

pub const SERVER_MANIFEST_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["js", "package.json"].iter().collect());
pub const SERVER_SOURCE_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["js", "server.js"].iter().collect());
pub const GITIGNORE_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| [".gitignore"].iter().collect());

pub fn mk_js(canonical_path: &Utf8PathBuf, name: &PackageName, _config: &Config) -> Result<()> {
    // Create the `package.json` file.
    let filename = canonical_path.join(SERVER_MANIFEST_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            formatdoc! {r#"
                {{
                    "name": "js",
                    "version": "0.1.0",
                    "description": "{name}-rpc-server",
                    "dependencies": {{
                        "express": "^4.18.2"
                    }}
                }}
            "#},
        )?;
    }

    // Create the `server.js` file.
    let filename = canonical_path.join(SERVER_SOURCE_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            indoc! {r#"
                const express = require('express');
                const app = express();
                const hostname = '127.0.0.1';
                const port = 3000;

                app.use(express.json());

                app.post('/sqrt', (req, res) => {
                    console.dir(`received payload ${JSON.stringify(req.body)}`);
                    n = Math.sqrt(req.body.n);
                    res.statusCode = 200;
                    res.setHeader('Content-Type', 'application/json');
                    res.end(JSON.stringify({ result: {n : Math.trunc(n)} }));
                });

                app.listen(port, hostname, () => {
                    console.log(`Example app listening on port ${port}`);
                });
            "#},
        )?;
    }

    // Create the `.gitignore` file.
    let filename = canonical_path.join(GITIGNORE_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            indoc! {r#"
                node_modules
            "#},
        )?;
    }

    Ok(())
}
