import { Column, PrimaryGeneratedColumn, Table } from '@immich/sql-tools';

@Table({ name: 'album' })
export class AlbumTable {
  @PrimaryGeneratedColumn()
  id!: string;

  @Column()
  albumName!: string;

  @Column()
  ownerId!: string;
}
