import { Column, Table } from '@immich/sql-tools';

@Table('asset')
export class AssetTable {
  @Column()
  id!: string;
}
