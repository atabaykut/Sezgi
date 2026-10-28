import { Column, ForeignKeyColumn, Table } from '@immich/sql-tools';
import { AssetTable } from './asset.table';

@Table({ name: 'album' })
export class AlbumTable {
  @Column()
  id!: string;

  @ForeignKeyColumn(() => AssetTable, { nullable: true, onDelete: 'SET NULL', onUpdate: 'CASCADE' })
  albumThumbnailAssetId!: string | null;
}
