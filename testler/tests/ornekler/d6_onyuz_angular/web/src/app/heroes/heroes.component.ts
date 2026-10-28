import { Component, OnInit } from '@angular/core';
import { RouterLink } from '@angular/router';
import { Hero, HeroService } from '../hero.service';

@Component({
  selector: 'app-heroes',
  templateUrl: './heroes.component.html',
  imports: [RouterLink],
})
export class HeroesComponent implements OnInit {
  heroes: Hero[] = [];

  constructor(private heroService: HeroService) {}

  ngOnInit(): void {
    this.heroService.getHeroes().subscribe(h => (this.heroes = h));
  }

  add(name: string): void {
    this.heroService.addHero({ name }).subscribe(h => this.heroes.push(h));
  }

  delete(hero: Hero): void {
    this.heroService.deleteHero(hero.id).subscribe();
  }
}
