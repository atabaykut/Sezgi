import { BadRequestException } from '@nestjs/common';

export const erisimIste = async (izinliler: Set<string>, ids: string[]) => {
  if (!ids.every((id) => izinliler.has(id))) {
    throw new BadRequestException('Not found or no access');
  }
};
