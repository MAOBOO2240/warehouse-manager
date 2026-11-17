// 僅供開發預覽：用記憶體裡的假後端在瀏覽器跑前端畫面（不需要 Rust / Tauri）。
//   npm run dev  →  http://localhost:5173/dev/preview.html
// 網址參數：?reset 重新產生示範資料；?empty 從空白專案開始；?update=1 模擬有新版本；?update=fail 模擬下載安裝失敗；
//           ?update=offline 模擬連不上 GitHub；?update=private 模擬私人倉庫沒有更新權杖（查不到發行版本）；
//           ?update=rebuild 模擬同一個版本重新編譯過（例如換了更新權杖）
// 這個檔案不會被打包進正式版（正式版只打包 index.html）。規則盡量比照 src-tauri/src/engine/tx.rs。
import { mockIPC, mockWindows } from '@tauri-apps/api/mocks'

const params = new URLSearchParams(location.search)
const DB_KEY = 'wm-mock-db-v2'
const MAX_QTY = 1_000_000_000
const DEFAULT_PROJECT = '預設總倉'

const clone = (v) => JSON.parse(JSON.stringify(v))
const fail = (code, message) => {
  throw { code, message }
}
const pad = (n, w = 2) => String(n).padStart(w, '0')
const isoUtc = (d) => d.toISOString().replace(/\.\d{3}Z$/, 'Z')
const localDay = (d) => `${d.getFullYear()}${pad(d.getMonth() + 1)}${pad(d.getDate())}`
const locName = (l) => (l ? l : '未指定')

function emptyProject(name) {
  return {
    config: { name, created_at: isoUtc(new Date()), schema_version: 1 },
    products: [],
    locations: [],
    inventory: [],
    logs: [],
    remarks: [],
    row_order: [],
  }
}

/* ---------------- 交易（比照 tx.rs 的 compute / run_transaction） ---------------- */

function checkQty(q) {
  if (!Number.isInteger(q) || q <= 0) fail('INVALID_INPUT', '數量必須是大於 0 的整數')
  if (q > MAX_QTY) fail('INVALID_INPUT', '數量過大，請確認輸入是否正確')
}
function ensureLocation(id) {
  if ([...id].length > 50) fail('INVALID_INPUT', '存放地點請在 50 個字以內')
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f]/.test(id) || id !== id.trim()) fail('INVALID_INPUT', '存放地點格式不正確')
}
const qtyAt = (inv, p, l) => inv.find((r) => r.product_id === p && r.location_id === l)?.quantity ?? 0
function setQty(inv, p, l, q, now) {
  const i = inv.findIndex((r) => r.product_id === p && r.location_id === l)
  if (i >= 0) {
    if (q === 0) inv.splice(i, 1)
    else Object.assign(inv[i], { quantity: q, last_updated: now })
  } else if (q !== 0) inv.push({ product_id: p, location_id: l, quantity: q, last_updated: now })
}

function applyTx(db, tx, when = new Date()) {
  const now = isoUtc(when)
  const prefix = `LOG-${localDay(when)}-`
  let next =
    db.logs.reduce((m, l) => (String(l.id).startsWith(prefix) ? Math.max(m, Number(l.id.slice(prefix.length)) || 0) : m), 0) + 1
  const inv = clone(db.inventory)
  const out = []
  const nameOf = (id) => {
    const p = db.products.find((x) => x.id === id)
    if (!p) fail('NOT_FOUND', `找不到商品 ${id}，可能已被刪除`)
    return p.name
  }
  const push = (action, pid, pname, loc, delta, after, reason, note) =>
    out.push({
      id: `${prefix}${pad(next++, 4)}`,
      timestamp: now,
      action_type: action,
      product_id: pid,
      product_name: pname,
      location_id: loc,
      delta,
      balance_after: after,
      reason,
      note: String(note ?? '').trim(),
    })
  const deduct = (action, verb, reason) => {
    checkQty(tx.quantity)
    const name = nameOf(tx.product_id)
    ensureLocation(tx.location_id)
    const before = qtyAt(inv, tx.product_id, tx.location_id)
    if (before < tx.quantity) fail('INSUFFICIENT_STOCK', `庫存不足！當前庫位剩餘 ${before} 件，不可${verb} ${tx.quantity} 件`)
    setQty(inv, tx.product_id, tx.location_id, before - tx.quantity, now)
    push(action, tx.product_id, name, tx.location_id, -tx.quantity, before - tx.quantity, reason, tx.note)
  }
  const extra = {}

  switch (tx.kind) {
    case 'INBOUND': {
      checkQty(tx.quantity)
      const name = nameOf(tx.product_id)
      ensureLocation(tx.location_id)
      const after = qtyAt(inv, tx.product_id, tx.location_id) + tx.quantity
      if (after > MAX_QTY) fail('INVALID_INPUT', '入庫後數量過大')
      setQty(inv, tx.product_id, tx.location_id, after, now)
      push('INBOUND', tx.product_id, name, tx.location_id, tx.quantity, after, String(tx.reason ?? '').trim() || '進貨入庫', tx.note)
      break
    }
    case 'OUTBOUND':
      deduct('OUTBOUND', '出庫', String(tx.reason ?? '').trim() || '出庫')
      break
    case 'DAMAGE': {
      const r = String(tx.reason ?? '').trim()
      if (!r) fail('INVALID_INPUT', '請選擇或填寫損壞原因')
      deduct('DAMAGE', '登記損壞', r)
      break
    }
    case 'TRANSFER': {
      checkQty(tx.quantity)
      if (tx.from_location_id === tx.to_location_id) fail('INVALID_INPUT', '新地點不可與原地點相同')
      const name = nameOf(tx.product_id)
      ensureLocation(tx.from_location_id)
      ensureLocation(tx.to_location_id)
      const fromBefore = qtyAt(inv, tx.product_id, tx.from_location_id)
      if (fromBefore < tx.quantity)
        fail('INSUFFICIENT_STOCK', `庫存不足！「${locName(tx.from_location_id)}」剩餘 ${fromBefore} 件，不可移動 ${tx.quantity} 件`)
      const toAfter = qtyAt(inv, tx.product_id, tx.to_location_id) + tx.quantity
      if (toAfter > MAX_QTY) fail('INVALID_INPUT', '移過去之後數量過大')
      setQty(inv, tx.product_id, tx.from_location_id, fromBefore - tx.quantity, now)
      setQty(inv, tx.product_id, tx.to_location_id, toAfter, now)
      push('TRANSFER', tx.product_id, name, tx.from_location_id, -tx.quantity, fromBefore - tx.quantity, `移動存放地點：移至 ${locName(tx.to_location_id)}`, tx.note)
      push('TRANSFER', tx.product_id, name, tx.to_location_id, tx.quantity, toAfter, `移動存放地點：由 ${locName(tx.from_location_id)} 移入`, tx.note)
      break
    }
    case 'REMOVE_LOCATION': {
      const from = tx.location_id
      const to = tx.to_location_id ?? ''
      if (!from) fail('INVALID_INPUT', '「未指定」不是存放地點，無法移除')
      if (from === to) fail('INVALID_INPUT', '不可移到正要移除的地點')
      ensureLocation(to)
      const moving = inv.filter((r) => r.location_id === from && r.quantity > 0).map((r) => [r.product_id, r.quantity])
      for (const [pid, q] of moving) {
        const name = db.products.find((x) => x.id === pid)?.name ?? pid
        const toAfter = qtyAt(inv, pid, to) + q
        if (toAfter > MAX_QTY) fail('INVALID_INPUT', `「${name}」移過去之後數量過大`)
        setQty(inv, pid, from, 0, now)
        setQty(inv, pid, to, toAfter, now)
        push('TRANSFER', pid, name, from, -q, 0, `移除存放地點：移至 ${locName(to)}`, tx.note)
        push('TRANSFER', pid, name, to, q, toAfter, `移除存放地點：由 ${locName(from)} 移入`, tx.note)
      }
      for (let i = inv.length - 1; i >= 0; i--) if (inv[i].location_id === from) inv.splice(i, 1)
      // 註記與排列順序跟著搬；記下移除時間，之後建議清單不再出現這個地點
      const remarks = []
      for (const r of db.remarks) {
        if (r.location_id !== from) remarks.push(clone(r))
      }
      for (const r of db.remarks.filter((x) => x.location_id === from)) {
        const hit = remarks.find((x) => x.product_id === r.product_id && x.location_id === to)
        if (!hit) remarks.push({ ...r, location_id: to })
        else if (r.remark && r.remark !== hit.remark) {
          // 接得下就接在後面；太長就把舊的那一則留在原處（不刪使用者寫的字）
          if ([...`${hit.remark}；${r.remark}`].length <= 200) hit.remark = `${hit.remark}；${r.remark}`
          else remarks.push(clone(r))
        }
      }
      const seen = new Set()
      const rowOrder = []
      for (const r of db.row_order) {
        const loc = r.location_id === from ? to : r.location_id
        const k = `${r.product_id}\u0000${loc}`
        if (r.location_id === from && db.row_order.some((x) => x.product_id === r.product_id && x.location_id === to)) continue
        if (seen.has(k)) continue
        seen.add(k)
        rowOrder.push({ product_id: r.product_id, location_id: loc })
      }
      const config = clone(db.config)
      config.removed_locations = { ...(config.removed_locations || {}), [from]: now }
      extra.config = config
      extra.remarks = remarks
      extra.row_order = rowOrder
      break
    }
    default:
      fail('INVALID_INPUT', `不支援的交易類型：${tx.kind}`)
  }
  const neg = inv.find((r) => r.quantity < 0)
  if (neg) fail('INSUFFICIENT_STOCK', `交易已中止：商品 ${neg.product_id} 於庫位 ${neg.location_id} 將變成負庫存`)

  db.inventory = inv
  db.logs.push(...out)
  let products = null
  if (tx.kind === 'INBOUND') {
    const p = db.products.find((x) => x.id === tx.product_id)
    if (p && !(p.first_qty > 0)) {
      p.first_qty = tx.quantity
      products = clone(db.products)
    }
  }
  if (extra.config) Object.assign(db, { config: extra.config, remarks: extra.remarks, row_order: extra.row_order })
  return { inventory: clone(inv), new_logs: clone(out), products, config: extra.config ?? null, remarks: extra.remarks ?? null, row_order: extra.row_order ?? null }
}

/* ---------------- 示範資料 ---------------- */

function seedDemo() {
  const db = emptyProject(DEFAULT_PROJECT)
  const names = ['活動海報 A2', '角色壓克力立牌', '紀念徽章（金）', '紀念徽章（銀）', '透明貼紙組', '明信片套組', '帆布提袋', '限量 T 恤 M', '限量 T 恤 L', '展示用背板', '資料夾 A4', '掛軸（大）']
  names.forEach((name, i) => db.products.push({ id: `PROD-${pad(i + 1, 3)}`, name, first_qty: 0, created_at: isoUtc(new Date(Date.now() - 20 * 864e5)) }))
  const pid = (i) => `PROD-${pad(i, 3)}`
  const at = (daysAgo, h, m = 0) => {
    const d = new Date()
    d.setDate(d.getDate() - daysAgo)
    d.setHours(h, m, 0, 0)
    return d
  }
  // 先收集、再依時間先後套用（歷史紀錄要照時間順序）
  const ops = []
  const run = (when, tx) => ops.push({ when, tx })
  const inb = (d, h, p, loc, q, note = '') => run(at(d, h), { kind: 'INBOUND', product_id: pid(p), location_id: loc, quantity: q, reason: '入庫', note })
  const out = (d, h, p, loc, q, note = '') => run(at(d, h, 20), { kind: 'OUTBOUND', product_id: pid(p), location_id: loc, quantity: q, reason: '出貨', note })
  const dmg = (d, h, p, loc, q, reason) => run(at(d, h, 40), { kind: 'DAMAGE', product_id: pid(p), location_id: loc, quantity: q, reason, note: '' })
  const mov = (d, h, p, a, b, q) => run(at(d, h, 50), { kind: 'TRANSFER', product_id: pid(p), from_location_id: a, to_location_id: b, quantity: q, note: '' })

  inb(13, 9, 1, 'A 櫃', 200, '首批到貨')
  inb(13, 9, 2, 'A 櫃', 120)
  inb(13, 10, 3, 'B 櫃', 300)
  inb(13, 10, 4, 'B 櫃', 300)
  inb(12, 11, 5, '後倉第二排', 500)
  inb(12, 14, 6, '後倉第二排', 240)
  inb(11, 9, 7, '門市展示架', 60)
  inb(11, 9, 8, '', 80)
  inb(11, 10, 9, '', 80)
  out(10, 15, 1, 'A 櫃', 35)
  out(10, 16, 3, 'B 櫃', 48)
  mov(9, 11, 1, 'A 櫃', '門市展示架', 40)
  out(9, 15, 5, '後倉第二排', 120)
  dmg(9, 16, 2, 'A 櫃', 3, '道具受損')
  inb(8, 9, 10, '後倉第二排', 12)
  out(8, 14, 6, '後倉第二排', 60)
  out(7, 11, 7, '門市展示架', 14)
  inb(6, 10, 11, 'B 櫃', 150)
  out(6, 15, 4, 'B 櫃', 75)
  out(5, 13, 1, '門市展示架', 22)
  mov(5, 14, 5, '後倉第二排', 'A 櫃', 100)
  inb(4, 9, 1, 'A 櫃', 150, '第二批')
  out(4, 16, 2, 'A 櫃', 41)
  dmg(4, 17, 5, 'A 櫃', 6, '印刷錯誤')
  out(3, 10, 8, '', 26)
  out(3, 15, 9, '', 31)
  inb(2, 9, 12, '後倉第二排', 30)
  out(2, 14, 3, 'B 櫃', 66)
  out(2, 16, 11, 'B 櫃', 40)
  inb(1, 10, 6, '門市展示架', 48)
  out(1, 15, 1, 'A 櫃', 58)
  out(1, 16, 7, '門市展示架', 9)
  inb(0, 9, 3, 'B 櫃', 120, '補貨')
  out(0, 10, 5, 'A 櫃', 24)
  out(0, 11, 6, '後倉第二排', 18)
  dmg(0, 11, 12, '後倉第二排', 1, '道具受損')
  // 曾經用過、現在沒有庫存的地點
  inb(12, 16, 10, '舊倉庫', 4)
  mov(8, 10, 10, '舊倉庫', '後倉第二排', 4)
  // 今天還沒到的時段：改成剛剛發生（每筆間隔 3 分鐘），這樣不管幾點開預覽，「今日」都有資料
  const nowMs = Date.now()
  const future = ops.filter((o) => o.when.getTime() > nowMs)
  future.forEach((o, i) => (o.when = new Date(nowMs - (future.length - i) * 3 * 60000)))
  ops.sort((a, b) => a.when - b.when).forEach((o) => applyTx(db, o.tx, o.when))

  db.remarks.push({ product_id: pid(1), location_id: 'A 櫃', remark: '先出舊批' }, { product_id: pid(5), location_id: '後倉第二排', remark: '下層紙箱' })
  return db
}

function load() {
  if (!params.has('reset') && !params.has('empty')) {
    try {
      const raw = JSON.parse(localStorage.getItem(DB_KEY) || 'null')
      if (raw && raw.projects && Object.keys(raw.projects).length) return raw
    } catch {
      /* 重新建立 */
    }
  }
  const projects = params.has('empty') ? { [DEFAULT_PROJECT]: emptyProject(DEFAULT_PROJECT) } : { [DEFAULT_PROJECT]: seedDemo(), 分店倉: emptyProject('分店倉') }
  return { projects, last: DEFAULT_PROJECT }
}
const state = load()
const save = () => {
  try {
    localStorage.setItem(DB_KEY, JSON.stringify(state))
  } catch {
    /* 忽略 */
  }
}
save()
const list = () => Object.keys(state.projects).sort()
const proj = (name) => state.projects[name] ?? fail('NOT_FOUND', `找不到專案「${name}」`)
const MASTER = ['config', 'products', 'locations', 'remarks', 'row_order']

const handlers = {
  get_app_info: () => ({ root: 'C:\\示範\\倉儲管理\\projects', root_error: null, projects: list(), last_project: state.last, version: '1.0.0' }),
  get_projects: () => list(),
  create_project: ({ name }) => {
    const n = String(name ?? '').trim()
    if (!n) fail('INVALID_INPUT', '專案名稱不可空白')
    if (state.projects[n]) fail('INVALID_INPUT', `專案「${n}」已存在`)
    state.projects[n] = emptyProject(n)
    state.last = n
    return n
  },
  delete_project: ({ project }) => {
    proj(project)
    delete state.projects[project]
    if (!list().length) state.projects[DEFAULT_PROJECT] = emptyProject(DEFAULT_PROJECT)
    return list()
  },
  open_project: ({ project }) => {
    const p = proj(project)
    state.last = project
    return { name: project, data: clone(p), issues: [], recovered: false }
  },
  read_data: ({ project, fileType }) => clone(proj(project)[fileType]),
  write_data_atomic: ({ project, fileType, content }) => {
    if (!MASTER.includes(fileType)) fail('INVALID_INPUT', `${fileType}.json 不可直接寫入，請使用作業功能進行庫存異動`)
    if (fileType === 'products') {
      const seen = new Set()
      for (const it of content) {
        const k = String(it.name ?? '').trim().toLowerCase()
        if (!k) fail('INVALID_INPUT', `商品 ${it.id} 缺少名稱`)
        if (seen.has(k)) fail('INVALID_INPUT', `品項「${it.name}」已存在`)
        seen.add(k)
      }
    }
    proj(project)[fileType] = clone(content)
    return null
  },
  apply_transaction: ({ project, tx }) => applyTx(proj(project), tx),
  restore_backup: () => 'mock_20260101_000000_000.bak',
  export_text_file: ({ defaultName, content }) => {
    window.__wmLastExport = { name: defaultName, content }
    return `C:\\示範\\匯出\\${defaultName}`
  },
  export_project_zip: ({ project }) => `C:\\示範\\匯出\\${project}_備份.zip`,
  import_project_zip: () => null,
  clear_cache: () => 0,
  update_info: () => ({ version: '1.0.0', build: '57', commit: 'abc1234', can_update: true, repo_url: 'https://github.com/MAOBOO2240/warehouse-manager', has_token: false }),
  update_check: () => {
    // 訊息和 src-tauri/src/update.rs 的一樣
    if (params.get('update') === 'offline') throw '連不上 GitHub，請確認網路後再試'
    if (params.get('update') === 'private') throw '找不到發行版本。GitHub 倉庫如果是私人的，需要先設定更新權杖（UPDATE_TOKEN）重新編譯，或把倉庫改成公開'
    // ?update=rebuild：同一個版本重新編譯過（commit 一樣、編譯代號不同）
    if (params.get('update') === 'rebuild') {
      return { has_update: true, can_update: true, current: 'abc1234', latest: 'abc1234', latest_build: '57', latest_id: '200-1', rebuilt: true, published: new Date().toISOString() }
    }
    const has = params.has('update')
    return { has_update: has, can_update: true, current: 'abc1234', latest: has ? 'def5678' : 'abc1234', latest_build: has ? '58' : '57', latest_id: has ? '101-1' : '100-1', rebuilt: false, published: new Date().toISOString() }
  },
  update_install: async () => {
    await new Promise((r) => setTimeout(r, 2500))
    if (params.get('update') === 'fail') throw '下載中斷，請確認網路後再試'
    window.__wmUpdated = true
    return null
  },
}

mockWindows('main')
mockIPC(async (cmd, args) => {
  if (cmd.startsWith('plugin:')) {
    // 視窗標題列的深淺色等：記下來方便檢查，其他一律略過
    if (cmd === 'plugin:window|set_theme') window.__wmWindowTheme = args?.value ?? null
    return null
  }
  const h = handlers[cmd]
  if (!h) {
    console.warn('[mock] 未實作的指令', cmd, args)
    return null
  }
  const res = await h(args || {})
  save()
  return res
})
window.__wmMock = { state, applyTx, reset: () => (localStorage.removeItem(DB_KEY), location.reload()) }
console.info('[mock] 使用瀏覽器預覽用的假後端；資料存在 localStorage（網址加 ?reset 可重新產生）')
