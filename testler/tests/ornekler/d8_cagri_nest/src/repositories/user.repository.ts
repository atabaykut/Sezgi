import { Injectable } from '@nestjs/common';
import { Kysely } from 'kysely';
import { InjectKysely } from 'nestjs-kysely';

@Injectable()
export class UserRepository {
  constructor(@InjectKysely() private db: Kysely<any>) {}

  update(id: string, dto: any) {
    return this.db.updateTable('user').set(dto).where('id', '=', id).returningAll().executeTakeFirstOrThrow();
  }
}
