import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import './components/auth/auth.css'
import './styles/mobile.css'
import App from './App.jsx'
import { initMotion } from './utils/motion.js'

createRoot(document.getElementById('root')).render(
  <StrictMode>
    <App />
  </StrictMode>,
)

// Progressive enhancement only — the app is fully legible if this never runs.
initMotion()
