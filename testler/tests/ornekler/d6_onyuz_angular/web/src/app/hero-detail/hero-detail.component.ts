import { Component, inject } from '@angular/core';
import { Router } from '@angular/router';
import { Hero, HeroService } from '../hero.service';

@Component({
  selector: 'app-hero-detail',
  template: `
    <h2>Ayrıntı</h2>
    <form (ngSubmit)="save()">
      <input name="name" />
      <button type="submit">Save</button>
    </form>
    <a routerLink="/heroes">Back</a>
  `,
})
export class HeroDetailComponent {
  hero: Hero = { id: 1, name: '' };
  private heroService = inject(HeroService);
  private router = inject(Router);

  save(): void {
    this.heroService.updateHero(this.hero).subscribe(() => this.router.navigate(['/heroes']));
  }
}
