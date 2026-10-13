export const ACTIONS = {
  INBOUND: { label: '入庫', tone: 'go' },
  OUTBOUND: { label: '出貨', tone: 'steel' },
  TRANSFER: { label: '調撥', tone: 'mute' },
  ADJUST_PROFIT: { label: '盤盈', tone: 'go' },
  ADJUST_LOSS: { label: '盤虧', tone: 'signal' },
  SCRAP: { label: '報廢', tone: 'signal' },
  DAMAGE: { label: '損壞', tone: 'signal' },
}

export const TONE_CLASS = {
  go: 'bg-go-soft text-go',
  steel: 'bg-steel-soft text-steel',
  mute: 'bg-surface text-ink-3',
  signal: 'bg-signal-soft text-signal',
}

/** 目前操作介面使用的異動類型（流水帳篩選用） */
export const ACTIVE_ACTIONS = ['INBOUND', 'OUTBOUND', 'DAMAGE']

/** 存放地點顯示文字；空白代表未指定 */
export function locLabel(loc) {
  return loc ? loc : '未指定'
}

export function actionLabel(type) {
  return ACTIONS[type]?.label ?? type
}

const pad = (n) => String(n).padStart(2, '0')

/** ISO 時間 → 本機「2026/10/13 14:30」 */
export function fmtTime(iso) {
  if (!iso) return ''
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return iso
  return `${d.getFullYear()}/${pad(d.getMonth() + 1)}/${pad(d.getDate())} ${pad(d.getHours())}:${pad(d.getMinutes())}`
}

/** ISO 時間 → 本機日期「2026-10-13」（日期篩選用） */
export function localDate(iso) {
  const d = new Date(iso)
  if (Number.isNaN(d.getTime())) return ''
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`
}

export function todayStamp() {
  const d = new Date()
  return `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}_${pad(d.getHours())}${pad(d.getMinutes())}`
}

export function fmtInt(n) {
  return Number(n || 0).toLocaleString('zh-TW')
}

export function fmtMoney(n) {
  return Number(n || 0).toLocaleString('zh-TW', { minimumFractionDigits: 0, maximumFractionDigits: 2 })
}

export function fmtDelta(n) {
  return n > 0 ? `+${fmtInt(n)}` : fmtInt(n)
}

/** 解析正整數；不合法回傳 null */
export function parseQty(v) {
  const s = String(v ?? '').trim()
  if (!/^\d+$/.test(s)) return null
  const n = Number(s)
  return n > 0 && n <= 1_000_000_000 ? n : null
}

/** 解析 0 以上整數（盤點用）；空白回傳 null */
export function parseCount(v) {
  const s = String(v ?? '').trim()
  if (!/^\d+$/.test(s)) return null
  const n = Number(s)
  return n <= 1_000_000_000 ? n : null
}
