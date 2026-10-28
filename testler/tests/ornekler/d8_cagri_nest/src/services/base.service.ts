import { Injectable } from '@nestjs/common';
import { AccessRepository } from '../repositories/access.repository';
import { AlbumRepository } from '../repositories/album.repository';
import { UserRepository } from '../repositories/user.repository';
import { requireAccess } from '../utils/access';

@Injectable()
export class BaseService {
  constructor(
    protected accessRepository: AccessRepository,
    protected albumRepository: AlbumRepository,
    protected userRepository: UserRepository,
  ) {}

  requireAccess(request: { auth: any; permission: string; ids: string[] }) {
    return requireAccess(this.accessRepository, request);
  }
}
