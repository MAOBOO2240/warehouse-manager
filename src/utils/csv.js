// CSV 產生工具：UTF-8 BOM + CRLF，繁體中文 Windows Excel 直接開啟不會亂碼。
import { api } from '../api'

function escapeCell(value) {
  const s = value == null ? '' : String(value)
  return /[",\r\n]/.test(s) ? `"${s.replace(/"/g, '""')}"` : s
}

export function toCsv(headers, rows) {
  const lines = [headers, ...rows].map((row) => row.map(escapeCell).join(','))
  return '﻿' + lines.join('\r\n') + '\r\n'
}

/** 產生 CSV 並跳出另存新檔視窗。回傳儲存路徑，使用者取消時回傳 null。 */
export async function saveCsv(defaultName, headers, rows) {
  return api.exportTextFile(defaultName, toCsv(headers, rows))
}
