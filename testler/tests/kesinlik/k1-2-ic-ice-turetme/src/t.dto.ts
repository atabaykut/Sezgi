import { IntersectionType, OmitType, PartialType, PickType } from '@nestjs/swagger';
import { IsInt, IsOptional, IsString } from 'class-validator';
import { Bilinmeyen } from 'baska-bir-paket';

export class Temel {
  @IsString() id!: string;
  @IsString() ad!: string;
  @IsInt() yas!: number;
  @IsOptional() @IsString() not?: string;
}

export class Ek {
  @IsString() etiket!: string;
  @IsString() renk!: string;
}

// ad?, yas?, not?
export class A extends PartialType(OmitType(Temel, ['id'] as const)) {}
// ad?, yas?
export class B extends PickType(PartialType(Temel), ['ad', 'yas'] as const) {}
// id, ad, yas, not?, etiket
export class C extends IntersectionType(Temel, PickType(Ek, ['etiket'] as const)) {}
// id, ad, yas, etiket
export class D extends OmitType(IntersectionType(Temel, Ek), ['renk', 'not'] as const) {}
// Üst sınıf bu depoda yok: alanlar bilinemez
export class E extends PartialType(Bilinmeyen) {}
// Desteklenmeyen türetme fonksiyonu
export class F extends mixin(Temel) {}
// Üç argümanlı birleşim: id, ad, yas, not?, etiket, renk
export class G extends IntersectionType(Temel, Ek, PickType(Ek, ['renk'] as const)) {}
// Anahtar listesi değişkenden geliyor: hangi alanların seçildiği bilinemez
const SECILEN = ['ad'] as const;
export class H extends PickType(Temel, SECILEN) {}
