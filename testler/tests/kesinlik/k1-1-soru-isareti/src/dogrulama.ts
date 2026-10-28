import { applyDecorators } from '@nestjs/common';
import { IsOptional, IsString } from 'class-validator';

// Projedeki özel dekoratör: koşulsuz IsOptional içerir, alan isteğe bağlıdır
export const SecimliMetin = () => applyDecorators(IsOptional(), IsString());

// IsOptional yalnızca bir koşulda eklenir: zorunluluk argümana bağlı, koddan kesin çıkarılamaz
export function Kosullu(secenek?: { bos?: boolean }) {
  const d = [IsString()];
  if (secenek?.bos) {
    d.push(IsOptional());
  }
  return applyDecorators(...d);
}
