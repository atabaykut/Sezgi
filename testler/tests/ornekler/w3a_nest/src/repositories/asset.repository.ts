export class AssetRepository {
  constructor(private db: any) {}

  async remove(id: string) {
    await this.db.deleteFrom('asset').where('id', '=', id).execute();
  }

  async softDeleteAll(ids: string[], status: string) {
    await this.db.updateTable('asset').set({ status }).where('id', 'in', ids).execute();
  }
}
