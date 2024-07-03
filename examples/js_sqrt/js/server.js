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