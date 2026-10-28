import { Controller, Delete, Post } from '@nestjs/common';

@Controller('admin/database-backups')
export class BackupController {
  @Delete()
  deleteDatabaseBackup() {
    return [];
  }

  @Post('start-restore')
  startDatabaseRestoreFlow() {
    return [];
  }
}
