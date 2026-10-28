export class JobRepository {
  queue(item: unknown) {
    return item;
  }
  queueAll(items: unknown[]) {
    return items;
  }
}
