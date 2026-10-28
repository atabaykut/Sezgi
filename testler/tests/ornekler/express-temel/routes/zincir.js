// Zincirle kurulan router'lar: bağlamalar zincirin halkalarıdır
const express = require('express');
const etiketler = require('./etiket.controller');

const ic = express.Router().use(etiketler);

module.exports = express.Router().use('/v2', ic);
