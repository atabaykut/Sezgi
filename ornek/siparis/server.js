// Example order shop API for trying Sezgi. Run: npm install && npm start (http://127.0.0.1:3000)
const express = require('express');
const db = require('./db');

const app = express();
app.use(express.json());

app.use('/api/products', require('./routes/products'));
app.use('/api/orders', require('./routes/orders'));
app.use('/api/account', require('./routes/account'));
app.use('/api/users', require('./routes/users'));

const port = Number(process.env.PORT) || 3000;
db.open().then(() => {
  app.listen(port, '127.0.0.1', () => {
    console.log(`Example shop listening on http://127.0.0.1:${port}`);
    console.log(`Try: curl -H "Authorization: Bearer demo-customer-token" http://127.0.0.1:${port}/api/orders/1`);
  });
});
