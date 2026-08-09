import { createApp, h } from 'vue'
import { RouterView } from 'vue-router'
import { createPinia } from 'pinia'
import './style.css'
import router from './router'

const app = createApp({
  render: () => h(RouterView),
})
app.use(createPinia())
app.use(router)
app.mount('#app')
