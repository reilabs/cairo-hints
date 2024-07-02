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
                const crypto = require('crypto');
                const app = express();
                const hostname = '127.0.0.1';
                const port = 3000;

                app.use(express.json());

                // In-memory storage for job status (in a production environment, use a database)
                const jobs = new Map();

                app.post('/sqrt', (req, res) => {
                    console.dir(`received payload ${JSON.stringify(req.body)}`);
                    
                    // Generate a unique job ID
                    const jobId = crypto.randomBytes(16).toString('hex');

                    // Start the job
                    jobs.set(jobId, {status: 'processing'});

                    // Simulate a long-running process
                    setTimeout(() => {
                        const n = Math.sqrt(BigInt(req.body.felt252_n));
                        const result = { n: Math.trunc(n).toString() };
                        jobs.set(jobId, {status: 'completed', result: result});
                        console.log(`Job ${jobId} completed: ${JSON.stringify(result, null, 2)}`);
                    }, 5000); // Simulate a 5-second process

                    // Immediately return the job ID
                    res.json({ jobId });
                });

                app.get('/status/:jobId', (req, res) => {
                    const jobId = req.params.jobId;
                    const job = jobs.get(jobId);

                    if (!job) {
                        return res.status(404).json({ error: 'Job not found' });
                    }

                    if (job.status === 'completed') {
                        return res.json({ status: 'completed', result: job.result });
                    } else {
                        return res.json({ status: 'processing' });
                    }
                });

                app.listen(port, hostname, () => {
                    console.log(`Sqrt server listening on port ${port}`);
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
