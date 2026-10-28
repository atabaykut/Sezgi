import { Column, PrimaryGeneratedColumn, Table } from '@immich/sql-tools';

@Table({ name: 'user' })
export class UserTable {
  @PrimaryGeneratedColumn()
  id!: string;

  @Column()
  email!: string;
}
