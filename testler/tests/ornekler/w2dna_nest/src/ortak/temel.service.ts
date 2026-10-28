import { erisimIste } from './erisim';

export class TemelService {
  erisimIste(ids: string[]) {
    return erisimIste(new Set(), ids);
  }
}
