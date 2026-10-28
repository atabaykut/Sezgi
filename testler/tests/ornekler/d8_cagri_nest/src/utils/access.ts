import { ForbiddenException } from '@nestjs/common';
import { AccessRepository } from '../repositories/access.repository';

export const requireAccess = async (access: AccessRepository, request: { auth: any; permission: string; ids: string[] }) => {
  const izinli = await access.checkAlbumAccess(request.auth.user.id, request.ids);
  if (izinli.length !== request.ids.length) {
    throw new ForbiddenException('Not found or no access');
  }
};
