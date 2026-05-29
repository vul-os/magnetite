import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import './index.css'
import './components/auth/auth.css'
import './styles/mobile.css'
import App from './App.jsx'

createRoot(document.getElementById('root')).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
