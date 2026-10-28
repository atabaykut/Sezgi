<script setup lang="ts">
import { RouterLink, RouterView } from 'vue-router';
</script>

<template>
  <nav>
    <RouterLink to="/items">Ürünler</RouterLink>
  </nav>
  <RouterView />
</template>
