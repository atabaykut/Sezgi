import { Injectable } from '@nestjs/common';
import { Kysely } from 'kysely';
import { InjectKysely } from 'nestjs-kysely';

@Injectable()
export class AlbumRepository {
  constructor(@InjectKysely() private db: Kysely<any>) {}

  getById(id: string) {
    return this.db
      .selectFrom('album')
      .innerJoin('album_user', 'album_user.albumId', 'album.id')
      .selectAll('album')
      .where('album.id', '=', id)
      .executeTakeFirst();
  }

  delete(id: string) {
    return this.db.deleteFrom('album').where('id', '=', id).execute();
  }
}
