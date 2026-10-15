// 數量欄位的簡易算式：支援 + - × ÷（* / x）、括號與小數，結果必須是正整數
// 不使用 eval，以遞迴下降解析，只接受數字與運算符號

const MAX_QTY = 1_000_000_000

/** 全形字元與常見乘除符號轉成半形，移除空白與千分位逗號 */
function normalize(text) {
  return String(text ?? '')
    .replace(/[０-９]/g, (c) => String.fromCharCode(c.charCodeAt(0) - 0xfee0))
    .replace(/[＋]/g, '+')
    .replace(/[－−–—]/g, '-')
    .replace(/[×＊xX✕]/g, '*')
    .replace(/[÷／]/g, '/')
    .replace(/[（]/g, '(')
    .replace(/[）]/g, ')')
    .replace(/[．。]/g, '.')
    .replace(/[\s,，]/g, '')
}

function evaluate(src) {
  let i = 0
  const peek = () => src[i]
  const fail = () => {
    throw new Error('syntax')
  }

  function number() {
    const m = /^\d+(\.\d+)?|^\.\d+/.exec(src.slice(i))
    if (!m) fail()
    i += m[0].length
    return Number(m[0])
  }
  function factor() {
    const c = peek()
    if (c === '+') {
      i++
      return factor()
    }
    if (c === '-') {
      i++
      return -factor()
    }
    if (c === '(') {
      i++
      const v = expr()
      if (peek() !== ')') fail()
      i++
      return v
    }
    return number()
  }
  function term() {
    let v = factor()
    while (peek() === '*' || peek() === '/') {
      const op = src[i++]
      const r = factor()
      if (op === '/') {
        if (r === 0) throw new Error('div0')
        v /= r
      } else v *= r
    }
    return v
  }
  function expr() {
    let v = term()
    while (peek() === '+' || peek() === '-') {
      const op = src[i++]
      const r = term()
      v = op === '+' ? v + r : v - r
    }
    return v
  }

  const v = expr()
  if (i !== src.length) fail()
  return v
}

const fmt = (n) => (Number.isInteger(n) ? n.toLocaleString('en-US') : String(Math.round(n * 1e6) / 1e6))

/**
 * 解析數量欄位
 * @returns {{ qty: number|null, isExpr: boolean, result: number|null, error: string }}
 */
export function analyzeQty(text) {
  const src = normalize(text)
  const out = { qty: null, isExpr: false, result: null, error: '' }
  if (!src) return out
  out.isExpr = /[+\-*/()]/.test(src.replace(/^\+/, ''))
  let v
  try {
    v = evaluate(src)
  } catch (e) {
    out.error = e.message === 'div0' ? '算式不能除以 0' : out.isExpr ? '算式有誤，請檢查' : '請輸入大於 0 的整數'
    return out
  }
  if (!Number.isFinite(v)) {
    out.error = '算式有誤，請檢查'
    return out
  }
  // 浮點誤差（例如 0.1*30）視為整數
  const r = Math.abs(v - Math.round(v)) < 1e-9 ? Math.round(v) : v
  out.result = r
  if (!Number.isInteger(r)) out.error = out.isExpr ? `計算結果 = ${fmt(r)}，數量需為整數` : '請輸入大於 0 的整數'
  else if (r <= 0) out.error = out.isExpr ? `計算結果 = ${fmt(r)}，數量需大於 0` : '請輸入大於 0 的整數'
  else if (r > MAX_QTY) out.error = '數量過大'
  else out.qty = r
  return out
}
