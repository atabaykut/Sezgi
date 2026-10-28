import { Component } from '@angular/core';
import { Router } from '@angular/router';
import { AuthService } from '../auth.service';

@Component({
  selector: 'app-login',
  templateUrl: './login.component.html',
})
export class LoginComponent {
  constructor(
    private readonly auth: AuthService,
    private readonly router: Router,
  ) {}

  login(): void {
    this.auth.login('a', 'b').subscribe(() => this.router.navigateByUrl('/heroes'));
  }
}
