import { ForbiddenException, Injectable } from '@nestjs/common';
import { BaseService } from './base.service';

@Injectable()
export class UserAdminService extends BaseService {
  async delete(auth: any, id: string) {
    if (auth.user.id === id) {
      throw new ForbiddenException('Cannot delete your own account');
    }
    return this.userRepository.update(id, { deletedAt: new Date() });
  }
}
