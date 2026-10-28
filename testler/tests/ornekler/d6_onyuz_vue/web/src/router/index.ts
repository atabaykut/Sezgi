import { createRouter, createWebHistory } from 'vue-router';
import ItemsView from '../views/ItemsView.vue';

export default createRouter({
  history: createWebHistory(),
  routes: [
    { path: '/items', component: ItemsView },
    { path: '/admin', component: () => import('../views/AdminView.vue'), meta: { requiresAuth: true } },
  ],
});
