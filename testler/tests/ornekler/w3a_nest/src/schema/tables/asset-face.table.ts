import { Column, ForeignKeyColumn, Table } from '@immich/sql-tools';
import { AssetTable } from './asset.table';

@Table({ name: 'asset_face' })
export class AssetFaceTable {
  @Column()
  id!: string;

  @ForeignKeyColumn(() => AssetTable, {
    onDelete: 'CASCADE',
    onUpdate: 'CASCADE',
  })
  assetId!: string;
}
