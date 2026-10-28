import { Component } from '@angular/core';
import { HttpClient } from '@angular/common/http';

@Component({
  selector: 'app-admin-dashboard',
  template: `<button (click)="load()">Refresh stats</button> <a routerLink="stats">Stats</a>`,
})
export class AdminDashboardComponent {
  constructor(private http: HttpClient) {}

  load(): void {
    this.http.get('api/admin/stats').subscribe();
  }
}
