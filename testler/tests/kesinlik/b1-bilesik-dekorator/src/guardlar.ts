import { CanActivate, Injectable } from '@nestjs/common';

@Injectable()
export class AccessGuard implements CanActivate {
  canActivate() {
    return true;
  }
}

@Injectable()
export class ScopeGuard implements CanActivate {
  canActivate() {
    return true;
  }
}
