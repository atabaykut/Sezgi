import { Injectable, signal } from '@angular/core';
import { HttpClient } from '@angular/common/http';

@Injectable({ providedIn: 'root' })
export class AuthService {
  isLoggedIn = signal(false);

  constructor(private http: HttpClient) {}

  login(user: string, password: string) {
    return this.http.post('/api/auth/login', { user, password });
  }
}
