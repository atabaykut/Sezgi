import { Kysely, sql } from 'kysely';

export async function up(db: Kysely<any>): Promise<void> {
  await sql`ALTER TABLE "assets" RENAME TO "asset";`.execute(db);
  await sql`ALTER TABLE "users" RENAME TO "user";`.execute(db);
  await sql`ALTER TABLE "albums_assets" RENAME TO "album_asset";`.execute(db);
}
