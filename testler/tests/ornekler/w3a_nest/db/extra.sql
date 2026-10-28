CREATE TABLE IF NOT EXISTS "tag_asset" (
  "tagId" uuid NOT NULL,
  "assetId" uuid NOT NULL REFERENCES "asset" ("id") ON DELETE CASCADE
);
