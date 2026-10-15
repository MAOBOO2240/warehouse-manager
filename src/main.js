import { createApp } from 'vue'
import { createPinia } from 'pinia'
import App from './App.vue'
import './style.css'

// 停用右鍵選單與瀏覽器快捷鍵造成的重新整理，讓它表現得像一般桌面程式
window.addEventListener('contextmenu', (e) => {
  if (!(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement)) e.preventDefault()
})
window.addEventListener('keydown', (e) => {
  if (e.key === 'F5' || (e.ctrlKey && e.key.toLowerCase() === 'r')) e.preventDefault()
})

// 關閉瀏覽器記憶的輸入建議（已儲存資訊）；需要建議清單的欄位改用程式內的自訂選單
const noAutofill = (el) => {
  if ((el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) && el.getAttribute('autocomplete') !== 'off') {
    el.setAttribute('autocomplete', 'off')
  }
}
document.addEventListener('focusin', (e) => noAutofill(e.target), true)
document.addEventListener('pointerdown', (e) => noAutofill(e.target), true)

createApp(App).use(createPinia()).mount('#app')
