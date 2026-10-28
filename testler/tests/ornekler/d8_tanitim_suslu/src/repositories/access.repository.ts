import { Injectable } from '@nestjs/common';
import { Kysely } from 'kysely';
import { InjectKysely } from 'nestjs-kysely';

@Injectable()
export class AccessRepository {
  constructor(@InjectKysely() private db: Kysely<any>) {}

  checkAlbumAccess(userId: string, ids: string[]) {
    return this.db.selectFrom('album_user').select('albumId').where('userId', '=', userId).where('albumId', 'in', ids).execute();
  }
}
