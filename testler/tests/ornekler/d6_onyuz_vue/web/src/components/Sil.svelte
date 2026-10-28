<script>
  import axios from 'axios';

  function sil() {
    axios.delete('/api/items/1');
  }
</script>

<button on:click={sil}>Sil</button>

<style>
  button { color: red; }
</style>
