import express from 'express';
import { dataSource, Note } from './db';

const app = express();
app.use(express.json());

app.post('/api/notes/purge', async (req, res) => {
  await dataSource.createQueryBuilder().delete().from(Note).where('done = :d', { d: true }).execute();
  res.sendStatus(204);
});

app.post('/api/notes/clear', async (req, res) => {
  await dataSource
    .createQueryBuilder()
    .delete()
    .from(Note)
    .execute();
  res.sendStatus(204);
});

app.listen(3000);
