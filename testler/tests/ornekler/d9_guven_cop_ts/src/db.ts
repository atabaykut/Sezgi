import { DataSource, Entity, PrimaryGeneratedColumn, Column } from 'typeorm';

@Entity('notes')
export class Note {
  @PrimaryGeneratedColumn()
  id!: number;

  @Column()
  done!: boolean;
}

export const dataSource = new DataSource({ type: 'postgres', entities: [Note] });
