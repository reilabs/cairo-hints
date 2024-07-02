use crate::fsx;
use anyhow::Result;
use camino::Utf8PathBuf;
use indoc::{formatdoc, indoc};
use once_cell::sync::Lazy;
use scarb::core::{Config, PackageName};

pub const SERVER_MANIFEST_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["ts", "package.json"].iter().collect());
pub const SERVER_SOURCE_PATH: Lazy<Utf8PathBuf> =
    Lazy::new(|| ["ts/src", "index.ts"].iter().collect());
pub const GITIGNORE_PATH: Lazy<Utf8PathBuf> = Lazy::new(|| [".gitignore"].iter().collect());
pub const TSCONFIG_PATH: Lazy<Utf8PathBuf> = Lazy::new(|| ["ts", "tsconfig.json"].iter().collect());

pub fn mk_ts(canonical_path: &Utf8PathBuf, name: &PackageName, _config: &Config) -> Result<()> {
    // Create the `package.json` file.
    let filename = canonical_path.join(SERVER_MANIFEST_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            formatdoc! {r#"
                {{
                    "name": "ts",
                    "version": "0.1.0",
                    "description": "{name}-rpc-server",
                    "scripts": {{
                        "build": "tsc",
                        "start": "node build/index.js",
                        "dev": "ts-node src/index.ts"
                    }},
                    "dependencies": {{
                        "express": "^4.18.2"
                    }},
                    "devDependencies": {{
                        "typescript": "^4.0.0",
                        "@types/node": "^14.0.0",
                        "@types/express": "^4.17.0",
                        "ts-node": "^10.0.0"
                    }}
                }}      
            "#},
        )?;
    }

    // Create the `tsconfig.json` file.
    let filename = canonical_path.join(TSCONFIG_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            indoc! {r#"
                {
                    "compilerOptions": {
                        "target": "es6",
                        "module": "commonjs",
                        "outDir": "./build",
                        "rootDir": "./src",
                        "strict": true,
                        "esModuleInterop": true,
                        "skipLibCheck": true
                    },
                    "include": ["src/**/*"]
                }
            "#},
        )?;
    }

    // Create the `server.ts` file.
    let filename = canonical_path.join(SERVER_SOURCE_PATH.as_path());
    if !filename.exists() {
        fsx::create_dir_all(filename.parent().unwrap())?;

        fsx::write(
            filename,
            indoc! {r#"
                    import express, { Request, Response } from 'express';
                    import crypto from 'crypto';

                    const app = express();
                    const hostname: string = '127.0.0.1';
                    const port: number = 3000;

                    app.use(express.json());

                    // In-memory storage for job status (in a production environment, use a database)
                    const jobs: Map<string, {status: string, result?: any}> = new Map();

                    app.post('/sqrt', (req: Request, res: Response) => {
                        console.dir(`received payload ${JSON.stringify(req.body)}`);

                        // Generate a unique job ID
                        const jobId = crypto.randomBytes(16).toString('hex');

                        // Start the job
                        jobs.set(jobId, { status: 'processing' });

                        // Simulate a long-running process by uncommenting `setTimeout`
                        //setTimeout(() => {
                            const n = Math.sqrt(req.body.n);
                            const result = { n: Math.trunc(n) };
                            jobs.set(jobId, { status: 'completed', result: result });
                            console.log(`Job ${jobId} completed: ${JSON.stringify(result, null, 2)}`);
                        //}, 5000); // Simulate a 5-second process

                        // Immediately return the job ID
                        res.json({ jobId });
                    });

                    app.get('/status/:jobId', (req: Request, res: Response) => {
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
