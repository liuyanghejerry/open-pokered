import { createRouter, createWebHashHistory } from 'vue-router'
import App from './App.vue'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', redirect: '/map' },
    {
      path: '/:activity(map|script)',
      name: 'editor',
      component: App,
      props: (route) => ({
        routeActivity: route.params.activity as 'map' | 'script',
        routeQuery: route.query,
      }),
    },
    {
      path: '/save/:section?',
      name: 'save',
      component: App,
      props: (route) => ({
        routeActivity: 'save' as const,
        routeQuery: route.query,
      }),
    },
    {
      path: '/trainer/:className?',
      name: 'trainer',
      component: App,
      props: (route) => ({
        routeActivity: 'trainer' as const,
        routeQuery: route.query,
      }),
    },
    {
      path: '/pokemon/:species?',
      name: 'pokemon',
      component: App,
      props: (route) => ({
        routeActivity: 'pokemon' as const,
        routeQuery: route.query,
      }),
    },
    {
      path: '/move/:moveId?',
      name: 'move',
      component: App,
      props: (route) => ({
        routeActivity: 'move' as const,
        routeQuery: route.query,
      }),
    },
    {
      path: '/layout/:name?',
      name: 'layout',
      component: App,
      props: (route) => ({
        routeActivity: 'layout' as const,
        routeQuery: route.query,
      }),
    },
    {
      path: '/pixel/:asset?',
      name: 'pixel',
      component: App,
      props: (route) => ({
        routeActivity: 'pixel' as const,
        routeQuery: route.query,
      }),
    },
    {
      path: '/playtest',
      name: 'playtest',
      component: App,
      props: (route) => ({
        routeActivity: 'playtest' as const,
        routeQuery: route.query,
      }),
    },
  ],
})

export default router
