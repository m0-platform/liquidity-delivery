import { Buffer } from 'buffer'
window.Buffer = Buffer

import { createApp } from 'vue'
import App from './App.vue'
import './style.css'

// Wallet adapters initialize lazily in useWallet composable
// - wagmi (EVM): Browser extension wallets via injected connector
// - Solflare SDK (SVM): Solflare browser extension or web

createApp(App).mount('#app')
