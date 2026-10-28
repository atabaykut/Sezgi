import { Injectable } from '@angular/core';
import { HttpClient } from '@angular/common/http';
import { Observable } from 'rxjs';
import { environment } from 'src/environments/environment';

export interface Hero {
  id: number;
  name: string;
}

@Injectable({ providedIn: 'root' })
export class HeroService {
  private heroesUrl = `${environment.apiUrl}/heroes`;

  constructor(private http: HttpClient) {}

  getHeroes(): Observable<Hero[]> {
    return this.http.get<Hero[]>(this.heroesUrl);
  }

  getHero(id: number): Observable<Hero> {
    return this.http.get<Hero>(`${this.heroesUrl}/${id}`);
  }

  addHero(hero: Partial<Hero>): Observable<Hero> {
    return this.http.post<Hero>(this.heroesUrl, hero);
  }

  updateHero(hero: Hero): Observable<unknown> {
    return this.http.put(`${this.heroesUrl}/${hero.id}`, hero);
  }

  deleteHero(id: number): Observable<unknown> {
    return this.http.delete(`${this.heroesUrl}/${id}`);
  }
}
