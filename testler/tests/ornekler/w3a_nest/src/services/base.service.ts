import { AssetRepository } from '../repositories/asset.repository';
import { EventRepository } from '../repositories/event.repository';
import { JobRepository } from '../repositories/job.repository';

export class BaseService {
  constructor(
    protected assetRepository: AssetRepository,
    protected eventRepository: EventRepository,
    protected jobRepository: JobRepository,
  ) {}
}
