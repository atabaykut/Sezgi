import { inject, Injectable } from '@angular/core';
import { CanActivateFn } from '@angular/router';
import { AuthService } from './auth.service';

export const authGuard: CanActivateFn = () => inject(AuthService).isLoggedIn();

@Injectable({ providedIn: 'root' })
export class AuthGuard {
  canActivate(): boolean {
    return inject(AuthService).isLoggedIn();
  }
}
