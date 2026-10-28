import { OnEvent, OnJob } from '../decorators';
import { JobName } from '../enum';
import { BaseService } from './base.service';

export class TrashService extends BaseService {
  @OnEvent({ name: 'AssetDeleteAll' })
  async onAssetsDelete() {
    await this.jobRepository.queue({ name: JobName.AssetEmptyTrash, data: {} });
  }

  @OnJob({ name: JobName.AssetEmptyTrash })
  async handleEmptyTrash(ids: string[]) {
    await this.jobRepository.queueAll(ids.map((id) => ({ name: JobName.AssetDelete, data: { id } })));
  }
}
