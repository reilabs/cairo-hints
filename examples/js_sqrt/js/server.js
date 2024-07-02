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
    jobs.set(jobId, { status: 'processing' });

    //Uncomment this line to simulate long-running process
    //setTimeout(() => {
        const n = Math.sqrt(req.body.n);
        const result = { n: Math.trunc(n) };
        jobs.set(jobId, { status: 'completed', result: result });
        console.log(`Job ${jobId} completed: ${JSON.stringify(result, null, 2)}`);
    //}, 5000); // Simulate a 5-second process

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
