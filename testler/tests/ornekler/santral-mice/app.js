const express = require('express');

const app = express();
const router = express.Router();

router.post('/auth/logout', (req, res) => res.json({}));
router.get('/titles', (req, res) => res.json([]));

app.use('/mice/api', router);
app.get('/favicon.ico', (req, res) => res.end());
// Kısa bağlantı yönlendirmesi: tek parçalı yol
app.get('/:kod', (req, res) => res.redirect('/'));

app.listen(3000);
