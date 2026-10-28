import { Column, Table } from '@immich/sql-tools';

@Table({ name: 'album_user' })
export class AlbumUserTable {
  @Column()
  albumId!: string;

  @Column()
  userId!: string;
}
