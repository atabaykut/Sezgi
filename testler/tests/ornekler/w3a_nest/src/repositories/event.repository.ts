export class EventRepository {
  emit(name: string, data: unknown) {
    return [name, data];
  }
}
