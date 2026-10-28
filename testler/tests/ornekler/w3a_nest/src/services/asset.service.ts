import { OnJob } from '../decorators';
import { JobName } from '../enum';
import { BaseService } from './base.service';

export class AssetService extends BaseService {
  async deleteAll(ids: string[], force?: boolean) {
    await this.assetRepository.softDeleteAll(ids, force ? 'deleted' : 'trashed');
    await this.eventRepository.emit(force ? 'AssetDeleteAll' : 'AssetTrashAll', { ids });
  }

  @OnJob({ name: JobName.AssetDelete })
  async handleAssetDeletion(job: { id: string }) {
    await this.assetRepository.remove(job.id);
    await this.jobRepository.queue({ name: JobName.FileDelete, data: { id: job.id } });
  }
}
