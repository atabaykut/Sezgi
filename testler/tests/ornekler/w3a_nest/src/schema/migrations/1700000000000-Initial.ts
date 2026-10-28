import { Kysely, sql } from 'kysely';

export async function up(db: Kysely<any>): Promise<void> {
  await sql`CREATE TABLE "users" ("id" uuid NOT NULL PRIMARY KEY);`.execute(db);
  await sql`CREATE TABLE "assets" ("id" uuid NOT NULL PRIMARY KEY, "ownerId" uuid NOT NULL);`.execute(db);
  await sql`ALTER TABLE "assets" ADD CONSTRAINT "FK_owner" FOREIGN KEY ("ownerId") REFERENCES "users" ("id") ON UPDATE CASCADE ON DELETE CASCADE;`.execute(db);
  await sql`CREATE TABLE "albums_assets" ("albumsId" uuid NOT NULL, "assetsId" uuid NOT NULL REFERENCES "assets" ("id") ON DELETE CASCADE, PRIMARY KEY ("albumsId", "assetsId"));`.execute(db);
}
