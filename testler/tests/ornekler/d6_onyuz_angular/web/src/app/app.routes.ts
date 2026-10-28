import { inject } from '@angular/core';
import { Routes } from '@angular/router';
import { map } from 'rxjs/operators';
import { HeroesComponent } from './heroes/heroes.component';
import { HeroDetailComponent } from './hero-detail/hero-detail.component';
import { authGuard, AuthGuard } from './auth.guard';
import { AuthService } from './auth.service';

export const routes: Routes = [
  { path: '', redirectTo: '/heroes', pathMatch: 'full' },
  { path: 'heroes', component: HeroesComponent },
  { path: 'detail/:id', component: HeroDetailComponent, canActivate: [authGuard] },
  {
    path: 'login',
    loadComponent: () => import('./login/login.component').then(m => m.LoginComponent),
    canActivate: [() => inject(AuthService).isLoggedIn.pipe(map(girdi => !girdi))],
  },
  {
    path: 'admin',
    canActivate: [AuthGuard],
    loadChildren: () => import('./admin/admin.module').then(m => m.AdminModule),
  },
  { path: '**', component: HeroesComponent },
];
