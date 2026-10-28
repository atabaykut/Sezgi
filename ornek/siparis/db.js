// A tiny SQLite database that lives in memory (sql.js: pure WebAssembly, no native build).
// Call sites pass plain SQL text, which is also what lets Sezgi see which tables each endpoint touches.
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const initSqlJs = require('sql.js');

let raw;

function hashPassword(password) {
  const salt = crypto.randomBytes(16).toString('hex');
  return salt + ':' + crypto.scryptSync(password, salt, 32).toString('hex');
}

function checkPassword(password, stored) {
  const [salt, hash] = stored.split(':');
  const given = crypto.scryptSync(password, salt, 32);
  return crypto.timingSafeEqual(given, Buffer.from(hash, 'hex'));
}

async function open() {
  const SQL = await initSqlJs();
  raw = new SQL.Database();
  raw.run('PRAGMA foreign_keys = ON');
  raw.exec(fs.readFileSync(path.join(__dirname, 'schema.sql'), 'utf8'));
  seed();
}

function all(sql, params = []) {
  const stmt = raw.prepare(sql);
  stmt.bind(params);
  const rows = [];
  while (stmt.step()) rows.push(stmt.getAsObject());
  stmt.free();
  return rows;
}

function get(sql, params = []) {
  return all(sql, params)[0];
}

function run(sql, params = []) {
  raw.run(sql, params);
  const changes = raw.getRowsModified();
  const lastId = raw.exec('SELECT last_insert_rowid()')[0].values[0][0];
  return { changes, lastId };
}

// Demo data: one customer, one admin, three products, two orders.
function seed() {
  run('INSERT INTO users (id, email, role, api_token, password_hash) VALUES (?, ?, ?, ?, ?)',
    [1, 'customer@example.com', 'customer', 'demo-customer-token', hashPassword('customer-pass')]);
  run('INSERT INTO users (id, email, role, api_token, password_hash) VALUES (?, ?, ?, ?, ?)',
    [2, 'admin@example.com', 'admin', 'demo-admin-token', hashPassword('admin-pass')]);
  run('INSERT INTO products (id, name, price_cents) VALUES (1, ?, 1990), (2, ?, 4500), (3, ?, 899)',
    ['Coffee beans 1 kg', 'Pour-over kettle', 'Paper filters']);
  run('INSERT INTO orders (id, user_id, product_id, quantity, total_cents, status) VALUES (1, 1, 1, 2, 3980, ?), (2, 1, 3, 1, 899, ?)',
    ['paid', 'paid']);
}

module.exports = { open, all, get, run, hashPassword, checkPassword };
