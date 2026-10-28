<template>
  <button @click="ekle">Ekle</button>
</template>

<script setup lang="ts">
import axios from 'axios';

async function ekle() {
  await axios.post('/api/varliklar', { ad: 'x' });
}
</script>
