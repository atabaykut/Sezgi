<script lang="ts">
  import { varliklariSil, varlikGetir } from '@ornek/sdk';

  let { data } = $props();

  async function sil() {
    await varliklariSil({ dto: { ids: data.ids } });
  }

  async function goster(id: string) {
    return varlikGetir({ id });
  }
</script>

<h1>Fotoğraflar</h1>
<button onclick={sil}>Sil</button>
{#each data.ids as id}
  <span>{id}</span>
{/each}
