<script setup lang="ts">
import axios from 'axios';

async function kaydet() {
  await axios.post('/api/items', { ad: 'yeni' });
}
</script>

<template>
  <section>
    <button type="button" @click="kaydet">Kaydet</button>
    <router-link :to="'/admin'">Yönetim</router-link>
  </section>
</template>
