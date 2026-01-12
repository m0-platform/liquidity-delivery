import { createApp } from 'vue'
import App from './App.vue'
import './style.css'

// Initialize AppKit for devnet/mainnet modes
import { initializeAppKit } from './appkit'

const network = import.meta.env.VITE_NETWORK || 'local'

// Only initialize AppKit if not in local mode
if (network !== 'local') {
  initializeAppKit()
}

createApp(App).mount('#app')
