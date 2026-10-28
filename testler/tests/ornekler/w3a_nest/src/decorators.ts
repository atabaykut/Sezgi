import { SetMetadata } from '@nestjs/common';

export const OnEvent = (config: { name: string }) => SetMetadata('event', config);
export const OnJob = (config: { name: string }) => SetMetadata('job', config);
