import { Controller, Delete, Param } from '@nestjs/common';
import { UserAdminService } from '../services/user-admin.service';

@Controller('admin/users')
export class UserAdminController {
  constructor(private service: UserAdminService) {}

  @Delete(':id')
  deleteUserAdmin(@Param('id') id: string) {
    return this.service.delete({ user: { id: 'u' } }, id);
  }
}
