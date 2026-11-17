//! 庫存異動交易：入庫、出庫、調撥、盤點平倉、報廢、物品損壞、移除存放地點。
//!
//! 一筆交易的提交流程（run_transaction）：
//!   1. 讀取商品、庫位、庫存、日誌，於記憶體中計算新庫存與新日誌
//!   2. 任一庫位數量會小於 0 → 直接回傳 INSUFFICIENT_STOCK，不寫入任何檔案
//!   3. 將「新庫存全貌 + 新增日誌」原子寫入 pending_tx.json（交易日誌）
//!   4. 追加日誌 → 寫入庫存 → 寫入交易附帶的其他檔案 → 刪除 pending_tx.json
//! 若在第 4 步中途斷電，下次開啟專案時 commit_pending() 會依 pending_tx.json 補完，
//! 保證 inventory.json 與 logs.json 永遠一致。
//!
//! 移除存放地點（REMOVE_LOCATION）除了把該地點的庫存全部移走，還會一併更新三個檔案
//! （寫在交易日誌的 files 裡，和庫存、日誌一起補完）：
//!   - config.json 的 removed_locations 記下移除時間：歷史紀錄不可修改，仍會提到這個地點，
//!     前端依這個時間把它從「已使用過的存放地點」中隱藏；之後再用同一個名稱入庫就會重新出現
//!   - remarks.json、row_order.json 裡這個地點的註記與排列位置跟著搬到新地點

use super::error::{AppError, AppResult};
use super::storage::{self, file_path, read_array, write_json_atomic, PENDING_FILE};
use chrono::Local;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

pub const MAX_QTY: i64 = 1_000_000_000;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct InvRow {
    pub product_id: String,
    pub location_id: String,
    pub quantity: i64,
    #[serde(default)]
    pub last_updated: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LogEntry {
    pub id: String,
    pub timestamp: String,
    pub action_type: String,
    pub product_id: String,
    pub product_name: String,
    pub location_id: String,
    pub delta: i64,
    pub balance_after: i64,
    pub reason: String,
    pub note: String,
}

#[derive(Deserialize, Clone, Debug)]
pub struct StockCount {
    pub product_id: String,
    pub actual: i64,
}

#[derive(Deserialize, Clone, Debug)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TxRequest {
    Inbound {
        product_id: String,
        location_id: String,
        quantity: i64,
        #[serde(default)]
        reason: Option<String>,
        #[serde(default)]
        note: Option<String>,
    },
    Outbound {
        product_id: String,
        location_id: String,
        quantity: i64,
        #[serde(default)]
        reason: Option<String>,
        #[serde(default)]
        note: Option<String>,
    },
    Transfer {
        product_id: String,
        from_location_id: String,
        to_location_id: String,
        quantity: i64,
        #[serde(default)]
        note: Option<String>,
    },
    Stocktake {
        location_id: String,
        items: Vec<StockCount>,
        #[serde(default)]
        note: Option<String>,
    },
    Scrap {
        product_id: String,
        location_id: String,
        quantity: i64,
        reason: String,
        note: String,
    },
    Damage {
        product_id: String,
        location_id: String,
        quantity: i64,
        reason: String,
        #[serde(default)]
        note: String,
    },
    /// 移除存放地點：這個地點的庫存全部移到 to_location_id（空白＝未指定），
    /// 之後這個地點不再出現在清單與建議中
    RemoveLocation {
        location_id: String,
        #[serde(default)]
        to_location_id: String,
        #[serde(default)]
        note: Option<String>,
    },
}

#[derive(Serialize, Debug)]
pub struct TxOutcome {
    pub inventory: Vec<InvRow>,
    pub new_logs: Vec<LogEntry>,
    /// 首次入庫時會記錄商品的 first_qty，此時回傳更新後的商品清單
    pub products: Option<Vec<Value>>,
    /// 移除存放地點時一併更新的設定（removed_locations）；其他交易為 None
    pub config: Option<Value>,
    /// 移除存放地點時跟著搬的註記；其他交易為 None
    pub remarks: Option<Value>,
    /// 移除存放地點時跟著搬的排列順序；其他交易為 None
    pub row_order: Option<Value>,
}

/// 交易可以附帶更新的檔案（寫在交易日誌的 files 裡）
const SIDE_FILES: [&str; 3] = ["config", "remarks", "row_order"];
/// 註記的長度上限（和 validate_master_write 一致）
const REMARK_MAX: usize = 200;

fn text(v: &Option<String>) -> String {
    v.as_deref().unwrap_or("").trim().to_string()
}

fn check_qty(q: i64) -> AppResult<()> {
    if q <= 0 {
        return Err(AppError::invalid("數量必須是大於 0 的整數"));
    }
    if q > MAX_QTY {
        return Err(AppError::invalid("數量過大，請確認輸入是否正確"));
    }
    Ok(())
}

fn str_field<'a>(v: &'a Value, key: &str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or("")
}

struct Ctx<'a> {
    products: HashMap<&'a str, &'a str>,
    locations: HashSet<&'a str>,
}

impl<'a> Ctx<'a> {
    fn new(products: &'a [Value], locations: &'a [Value]) -> Self {
        Self {
            products: products
                .iter()
                .map(|p| (str_field(p, "id"), str_field(p, "name")))
                .collect(),
            locations: locations.iter().map(|l| str_field(l, "id")).collect(),
        }
    }
    fn product_name(&self, id: &str) -> AppResult<String> {
        self.products
            .get(id)
            .map(|s| s.to_string())
            .ok_or_else(|| AppError::not_found(format!("找不到商品 {id}，可能已被刪除")))
    }
    /// 存放地點為自由輸入（可空白代表「未指定」），不需預先建立；
    /// 若 locations.json 有舊資料也一律接受。
    fn ensure_location(&self, id: &str) -> AppResult<()> {
        if self.locations.contains(id) {
            return Ok(());
        }
        if id.chars().count() > 50 {
            return Err(AppError::invalid("存放地點請在 50 個字以內"));
        }
        if id.chars().any(|c| c.is_control()) || id != id.trim() {
            return Err(AppError::invalid("存放地點格式不正確"));
        }
        Ok(())
    }
}

fn loc_name(l: &str) -> &str {
    if l.is_empty() {
        "未指定"
    } else {
        l
    }
}

fn qty_at(inv: &[InvRow], p: &str, l: &str) -> i64 {
    inv.iter()
        .find(|r| r.product_id == p && r.location_id == l)
        .map(|r| r.quantity)
        .unwrap_or(0)
}

/// 設定某商品在某庫位的數量；歸零的列會移除，保持 inventory.json 精簡。
fn set_qty(inv: &mut Vec<InvRow>, p: &str, l: &str, q: i64, now: &str) {
    if let Some(i) = inv.iter().position(|r| r.product_id == p && r.location_id == l) {
        if q == 0 {
            inv.remove(i);
        } else {
            inv[i].quantity = q;
            inv[i].last_updated = now.to_string();
        }
    } else if q != 0 {
        inv.push(InvRow {
            product_id: p.to_string(),
            location_id: l.to_string(),
            quantity: q,
            last_updated: now.to_string(),
        });
    }
}

/// 產生 LOG-YYYYMMDD-0001 格式的流水號（日期為本機日期）
struct LogIds {
    prefix: String,
    next: u64,
}

impl LogIds {
    fn new(existing: &[Value], today: &str) -> Self {
        let prefix = format!("LOG-{today}-");
        let max = existing
            .iter()
            .filter_map(|l| str_field(l, "id").strip_prefix(prefix.as_str()))
            .filter_map(|n| n.parse::<u64>().ok())
            .max()
            .unwrap_or(0);
        Self { prefix, next: max + 1 }
    }
    fn take(&mut self) -> String {
        let id = format!("{}{:04}", self.prefix, self.next);
        self.next += 1;
        id
    }
}

struct LogBuilder<'a> {
    ids: LogIds,
    now: &'a str,
    out: Vec<LogEntry>,
}

impl LogBuilder<'_> {
    #[allow(clippy::too_many_arguments)]
    fn push(
        &mut self,
        action: &str,
        pid: &str,
        pname: &str,
        loc: &str,
        delta: i64,
        after: i64,
        reason: String,
        note: String,
    ) {
        let id = self.ids.take();
        self.out.push(LogEntry {
            id,
            timestamp: self.now.to_string(),
            action_type: action.to_string(),
            product_id: pid.to_string(),
            product_name: pname.to_string(),
            location_id: loc.to_string(),
            delta,
            balance_after: after,
            reason,
            note,
        });
    }
}

/// 純計算：依交易內容修改 `inventory`，回傳要追加的日誌。失敗時 `inventory` 內容不可再使用。
pub fn compute(
    tx: &TxRequest,
    products: &[Value],
    locations: &[Value],
    inventory: &mut Vec<InvRow>,
    existing_logs: &[Value],
    now: &str,
    today: &str,
) -> AppResult<Vec<LogEntry>> {
    let ctx = Ctx::new(products, locations);
    let mut logs = LogBuilder {
        ids: LogIds::new(existing_logs, today),
        now,
        out: Vec::new(),
    };

    match tx {
        TxRequest::Inbound {
            product_id,
            location_id,
            quantity,
            reason,
            note,
        } => {
            check_qty(*quantity)?;
            let name = ctx.product_name(product_id)?;
            ctx.ensure_location(location_id)?;
            let after = qty_at(inventory, product_id, location_id)
                .checked_add(*quantity)
                .filter(|v| *v <= MAX_QTY)
                .ok_or_else(|| AppError::invalid("入庫後數量過大"))?;
            set_qty(inventory, product_id, location_id, after, now);
            let reason = Some(text(reason))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "進貨入庫".into());
            logs.push(
                "INBOUND",
                product_id,
                &name,
                location_id,
                *quantity,
                after,
                reason,
                text(note),
            );
        }
        TxRequest::Outbound {
            product_id,
            location_id,
            quantity,
            reason,
            note,
        } => {
            let reason = Some(text(reason))
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "出庫".into());
            deduct(
                &ctx,
                &mut logs,
                inventory,
                "OUTBOUND",
                "出庫",
                product_id,
                location_id,
                *quantity,
                reason,
                text(note),
                now,
            )?;
        }
        TxRequest::Scrap {
            product_id,
            location_id,
            quantity,
            reason,
            note,
        } => {
            let (r, n) = required_reason(reason, note, "報廢")?;
            deduct(
                &ctx,
                &mut logs,
                inventory,
                "SCRAP",
                "報廢",
                product_id,
                location_id,
                *quantity,
                r,
                n,
                now,
            )?;
        }
        TxRequest::Damage {
            product_id,
            location_id,
            quantity,
            reason,
            note,
        } => {
            let r = reason.trim().to_string();
            if r.is_empty() {
                return Err(AppError::invalid("請選擇或填寫損壞原因"));
            }
            let n = note.trim().to_string();
            deduct(
                &ctx,
                &mut logs,
                inventory,
                "DAMAGE",
                "登記損壞",
                product_id,
                location_id,
                *quantity,
                r,
                n,
                now,
            )?;
        }
        TxRequest::Transfer {
            product_id,
            from_location_id,
            to_location_id,
            quantity,
            note,
        } => {
            check_qty(*quantity)?;
            if from_location_id == to_location_id {
                return Err(AppError::invalid("新地點不可與原地點相同"));
            }
            let name = ctx.product_name(product_id)?;
            ctx.ensure_location(from_location_id)?;
            ctx.ensure_location(to_location_id)?;
            let from_before = qty_at(inventory, product_id, from_location_id);
            if from_before < *quantity {
                return Err(AppError::insufficient(format!(
                    "庫存不足！「{}」剩餘 {from_before} 件，不可移動 {quantity} 件",
                    loc_name(from_location_id)
                )));
            }
            let from_after = from_before - quantity;
            let to_after = qty_at(inventory, product_id, to_location_id)
                .checked_add(*quantity)
                .filter(|v| *v <= MAX_QTY)
                .ok_or_else(|| AppError::invalid("移過去之後數量過大"))?;
            set_qty(inventory, product_id, from_location_id, from_after, now);
            set_qty(inventory, product_id, to_location_id, to_after, now);
            let note = text(note);
            logs.push(
                "TRANSFER",
                product_id,
                &name,
                from_location_id,
                -quantity,
                from_after,
                format!("移動存放地點：移至 {}", loc_name(to_location_id)),
                note.clone(),
            );
            logs.push(
                "TRANSFER",
                product_id,
                &name,
                to_location_id,
                *quantity,
                to_after,
                format!("移動存放地點：由 {} 移入", loc_name(from_location_id)),
                note,
            );
        }
        TxRequest::Stocktake {
            location_id,
            items,
            note,
        } => {
            ctx.ensure_location(location_id)?;
            if items.is_empty() {
                return Err(AppError::invalid("沒有任何盤點項目"));
            }
            let mut seen = HashSet::new();
            for it in items {
                if !seen.insert(it.product_id.as_str()) {
                    return Err(AppError::invalid(format!("商品 {} 在盤點單中重複", it.product_id)));
                }
                if it.actual < 0 || it.actual > MAX_QTY {
                    return Err(AppError::invalid("實盤數量必須是 0 以上的整數"));
                }
            }
            let note = text(note);
            for it in items {
                let name = ctx.product_name(&it.product_id)?;
                let book = qty_at(inventory, &it.product_id, location_id);
                let diff = it.actual - book;
                if diff == 0 {
                    continue;
                }
                set_qty(inventory, &it.product_id, location_id, it.actual, now);
                let action = if diff > 0 { "ADJUST_PROFIT" } else { "ADJUST_LOSS" };
                logs.push(
                    action,
                    &it.product_id,
                    &name,
                    location_id,
                    diff,
                    it.actual,
                    format!("盤點平倉（帳面 {book} → 實盤 {}）", it.actual),
                    note.clone(),
                );
            }
            if logs.out.is_empty() {
                return Err(AppError::invalid("實盤數量與帳面一致，沒有需要平倉的項目"));
            }
        }
        TxRequest::RemoveLocation {
            location_id,
            to_location_id,
            note,
        } => {
            if location_id.is_empty() {
                return Err(AppError::invalid("「未指定」不是存放地點，無法移除"));
            }
            if location_id == to_location_id {
                return Err(AppError::invalid("不可移到正要移除的地點"));
            }
            ctx.ensure_location(to_location_id)?;
            let note = text(note);
            // 這個地點所有有庫存的品項，依 inventory 原本的順序逐一移走（每個品項記一出一入）
            let moving: Vec<(String, i64)> = inventory
                .iter()
                .filter(|r| r.location_id == *location_id && r.quantity > 0)
                .map(|r| (r.product_id.clone(), r.quantity))
                .collect();
            for (pid, qty) in &moving {
                // 品項已不在清單裡（舊資料）時仍可移走，名稱以編號代替
                let name = ctx
                    .products
                    .get(pid.as_str())
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| pid.clone());
                let to_after = qty_at(inventory, pid, to_location_id)
                    .checked_add(*qty)
                    .filter(|v| *v <= MAX_QTY)
                    .ok_or_else(|| AppError::invalid(format!("「{name}」移過去之後數量過大")))?;
                set_qty(inventory, pid, location_id, 0, now);
                set_qty(inventory, pid, to_location_id, to_after, now);
                logs.push(
                    "TRANSFER",
                    pid,
                    &name,
                    location_id,
                    -qty,
                    0,
                    format!("移除存放地點：移至 {}", loc_name(to_location_id)),
                    note.clone(),
                );
                logs.push(
                    "TRANSFER",
                    pid,
                    &name,
                    to_location_id,
                    *qty,
                    to_after,
                    format!("移除存放地點：由 {} 移入", loc_name(location_id)),
                    note.clone(),
                );
            }
            // 舊資料可能留有數量為 0 的列，一併清掉；沒有庫存的地點不寫任何日誌。
            // 數量是負的列（資料異常）不在這裡默默刪掉，留給下面「不允許負數」那一關中止交易
            inventory.retain(|r| r.location_id != *location_id || r.quantity < 0);
        }
    }

    // 最後一道防線：任何庫位都不允許負數
    if let Some(r) = inventory.iter().find(|r| r.quantity < 0) {
        return Err(AppError::insufficient(format!(
            "交易已中止：商品 {} 於庫位 {} 將變成負庫存",
            r.product_id, r.location_id
        )));
    }
    Ok(logs.out)
}

fn required_reason(reason: &str, note: &str, label: &str) -> AppResult<(String, String)> {
    let (r, n) = (reason.trim(), note.trim());
    if r.is_empty() {
        return Err(AppError::invalid(format!("請填寫{label}原因")));
    }
    if n.is_empty() {
        return Err(AppError::invalid("請填寫責任備註（例如：發現人、責任歸屬）"));
    }
    Ok((r.to_string(), n.to_string()))
}

#[allow(clippy::too_many_arguments)]
fn deduct(
    ctx: &Ctx,
    logs: &mut LogBuilder,
    inventory: &mut Vec<InvRow>,
    action: &str,
    verb: &str,
    product_id: &str,
    location_id: &str,
    quantity: i64,
    reason: String,
    note: String,
    now: &str,
) -> AppResult<()> {
    check_qty(quantity)?;
    let name = ctx.product_name(product_id)?;
    ctx.ensure_location(location_id)?;
    let before = qty_at(inventory, product_id, location_id);
    if before < quantity {
        return Err(AppError::insufficient(format!(
            "庫存不足！當前庫位剩餘 {before} 件，不可{verb} {quantity} 件"
        )));
    }
    let after = before - quantity;
    set_qty(inventory, product_id, location_id, after, now);
    logs.push(action, product_id, &name, location_id, -quantity, after, reason, note);
    Ok(())
}

pub fn read_inventory(dir: &Path) -> AppResult<Vec<InvRow>> {
    read_array(dir, "inventory")?
        .into_iter()
        .map(|v| {
            serde_json::from_value::<InvRow>(v)
                .map_err(|e| AppError::corrupt(format!("inventory.json 內容格式不正確：{e}")))
        })
        .collect()
}

/// 執行交易（呼叫端須持有寫入鎖）
pub fn run_transaction(dir: &Path, tx: &TxRequest) -> AppResult<TxOutcome> {
    commit_pending(dir)?;
    let products = read_array(dir, "products")?;
    let locations = read_array(dir, "locations")?;
    let logs = read_array(dir, "logs")?;
    let mut inventory = read_inventory(dir)?;

    let now = storage::now_utc();
    let today = Local::now().format("%Y%m%d").to_string();
    let new_logs = compute(tx, &products, &locations, &mut inventory, &logs, &now, &today)?;

    let mut journal = json!({ "inventory": inventory, "new_logs": new_logs });
    // 移除存放地點：設定、註記、排列順序跟著更新，和庫存一起寫進交易日誌（中途斷電也會補完）
    let mut side = serde_json::Map::new();
    if let TxRequest::RemoveLocation {
        location_id,
        to_location_id,
        ..
    } = tx
    {
        let mut config = match storage::read_json(dir, "config")? {
            Value::Object(map) => map,
            _ => return Err(AppError::corrupt("config.json 內容格式不正確")),
        };
        mark_location_removed(&mut config, location_id, &now);
        side.insert("config".into(), Value::Object(config));
        // 註記與排列順序讀不到時不擋下移除（那兩個檔維持原狀）
        if let Ok(remarks) = read_array(dir, "remarks") {
            side.insert(
                "remarks".into(),
                Value::Array(move_remarks(remarks, location_id, to_location_id)),
            );
        }
        if let Ok(order) = read_array(dir, "row_order") {
            side.insert(
                "row_order".into(),
                Value::Array(move_row_order(order, location_id, to_location_id)),
            );
        }
        journal["files"] = Value::Object(side.clone());
    }
    write_json_atomic(dir, PENDING_FILE, &journal, false)?;
    commit_pending(dir)?;

    // 首次入庫：記錄該商品的入庫數量，之後選擇此商品時自動帶入
    let mut updated_products = None;
    if let TxRequest::Inbound {
        product_id, quantity, ..
    } = tx
    {
        let mut list = products;
        if let Some(p) = list.iter_mut().find(|p| str_field(p, "id") == product_id.as_str()) {
            let has_first = p.get("first_qty").and_then(Value::as_i64).unwrap_or(0) > 0;
            if !has_first {
                if let Some(obj) = p.as_object_mut() {
                    obj.insert("first_qty".into(), json!(quantity));
                }
                write_json_atomic(dir, "products", &Value::Array(list.clone()), true)?;
                updated_products = Some(list);
            }
        }
    }
    Ok(TxOutcome {
        inventory,
        new_logs,
        products: updated_products,
        config: side.remove("config"),
        remarks: side.remove("remarks"),
        row_order: side.remove("row_order"),
    })
}

/// 在設定裡記下「這個存放地點在什麼時候被移除」。
/// 歷史紀錄不可修改，仍會提到這個地點；前端只把「移除之後才發生的異動」算成用過這個地點。
fn mark_location_removed(config: &mut serde_json::Map<String, Value>, location: &str, now: &str) {
    let entry = config
        .entry("removed_locations")
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if !entry.is_object() {
        *entry = Value::Object(serde_json::Map::new());
    }
    if let Some(map) = entry.as_object_mut() {
        map.insert(location.to_string(), Value::String(now.to_string()));
    }
}

/// 註記跟著庫存搬到新地點：新地點沒有註記就直接搬過去；兩邊都有且內容不同時接在後面。
/// 接起來會超過長度上限時不硬湊：新地點保留原本的註記，舊的那一則原封不動留在檔案裡（不刪使用者寫的字）。
fn move_remarks(remarks: Vec<Value>, from: &str, to: &str) -> Vec<Value> {
    let (moving, mut kept): (Vec<Value>, Vec<Value>) = remarks
        .into_iter()
        .partition(|r| str_field(r, "location_id") == from);
    let mut left_behind = Vec::new();
    for m in moving {
        let pid = str_field(&m, "product_id").to_string();
        let text = str_field(&m, "remark").trim().to_string();
        if pid.is_empty() || text.is_empty() {
            continue;
        }
        let dest = kept
            .iter_mut()
            .find(|r| str_field(r, "product_id") == pid && str_field(r, "location_id") == to);
        match dest.and_then(Value::as_object_mut) {
            Some(obj) => {
                let old = obj
                    .get("remark")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if old.is_empty() {
                    obj.insert("remark".into(), Value::String(text));
                } else if old != text {
                    let joined = format!("{old}；{text}");
                    if joined.chars().count() <= REMARK_MAX {
                        obj.insert("remark".into(), Value::String(joined));
                    } else {
                        left_behind.push(m);
                    }
                }
            }
            None => kept.push(json!({ "product_id": pid, "location_id": to, "remark": text })),
        }
    }
    kept.extend(left_behind);
    kept
}

/// 排列順序跟著搬：被移除地點的列換成新地點，位置不變；新地點本來就有這個品項時，舊的那一筆直接拿掉。
fn move_row_order(order: Vec<Value>, from: &str, to: &str) -> Vec<Value> {
    let at_dest: HashSet<String> = order
        .iter()
        .filter(|r| str_field(r, "location_id") == to)
        .map(|r| str_field(r, "product_id").to_string())
        .collect();
    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(order.len());
    for r in order {
        let pid = str_field(&r, "product_id").to_string();
        let loc = str_field(&r, "location_id").to_string();
        if pid.is_empty() {
            continue;
        }
        let loc = if loc == from {
            if at_dest.contains(&pid) {
                continue;
            }
            to.to_string()
        } else {
            loc
        };
        if seen.insert((pid.clone(), loc.clone())) {
            out.push(json!({ "product_id": pid, "location_id": loc }));
        }
    }
    out
}

/// 若有未完成的交易日誌就補完。回傳是否有補完任何交易。可重複執行（冪等）。
pub fn commit_pending(dir: &Path) -> AppResult<bool> {
    let path = file_path(dir, PENDING_FILE);
    if !path.exists() {
        return Ok(false);
    }
    let journal = match storage::read_json(dir, PENDING_FILE) {
        Ok(v) => v,
        Err(_) => {
            // 交易日誌本身是原子寫入，理論上不會損壞；若真的損壞代表交易未曾生效，移除即可
            let _ = fs::remove_file(&path);
            return Ok(false);
        }
    };
    let inventory = journal.get("inventory").cloned().unwrap_or(Value::Null);
    let new_logs = journal
        .get("new_logs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !inventory.is_array() {
        let _ = fs::remove_file(&path);
        return Ok(false);
    }

    let mut logs = read_array(dir, "logs")?;
    let existing: HashSet<String> = logs.iter().map(|l| str_field(l, "id").to_string()).collect();
    let missing: Vec<Value> = new_logs
        .into_iter()
        .filter(|l| !existing.contains(str_field(l, "id")))
        .collect();
    if !missing.is_empty() {
        logs.extend(missing);
        write_json_atomic(dir, "logs", &Value::Array(logs), true)?;
    }
    write_json_atomic(dir, "inventory", &inventory, true)?;
    // 交易附帶的其他檔案（移除存放地點時的設定、註記、排列順序）；舊版寫下的交易日誌沒有這一段
    if let Some(files) = journal.get("files").and_then(Value::as_object) {
        for stem in SIDE_FILES {
            let Some(content) = files.get(stem) else { continue };
            let shape_ok = if stem == "config" {
                content.is_object()
            } else {
                content.is_array()
            };
            if shape_ok {
                write_json_atomic(dir, stem, content, true)?;
            }
        }
    }
    fs::remove_file(&path).map_err(|e| AppError::io("清除交易日誌失敗", &e))?;
    Ok(true)
}

/// 主檔（商品 / 庫位 / 設定）寫入前的檢查：格式、編號唯一、有庫存者不可刪除。
pub fn validate_master_write(dir: &Path, stem: &str, value: &Value) -> AppResult<()> {
    match stem {
        "config" => {
            if !value.is_object() {
                return Err(AppError::invalid("config 必須是物件"));
            }
            Ok(())
        }
        "products" | "locations" => {
            let items = value.as_array().ok_or_else(|| AppError::invalid("資料必須是陣列"))?;
            let is_product = stem == "products";
            let mut ids = HashSet::new();
            let mut skus = HashSet::new();
            let mut barcodes = HashSet::new();
            let mut names = HashSet::new();
            for it in items {
                let id = str_field(it, "id").trim();
                if id.is_empty() {
                    return Err(AppError::invalid("每筆資料都必須有編號"));
                }
                if !ids.insert(id) {
                    return Err(AppError::invalid(format!("編號 {id} 重複")));
                }
                if is_product {
                    let name = str_field(it, "name").trim();
                    if name.is_empty() {
                        return Err(AppError::invalid(format!("商品 {id} 缺少名稱")));
                    }
                    if !names.insert(name.to_lowercase()) {
                        return Err(AppError::invalid(format!("品項「{name}」已存在")));
                    }
                    let sku = str_field(it, "sku").trim();
                    if !sku.is_empty() && !skus.insert(sku) {
                        return Err(AppError::invalid(format!("SKU {sku} 重複")));
                    }
                    let bc = str_field(it, "barcode").trim();
                    if !bc.is_empty() && !barcodes.insert(bc) {
                        return Err(AppError::invalid(format!("條碼 {bc} 重複")));
                    }
                }
            }
            // 有庫存的商品 / 庫位不可刪除
            let inventory = read_inventory(dir)?;
            let old = read_array(dir, stem).unwrap_or_default();
            let old_name = |id: &str| -> String {
                old.iter()
                    .find(|o| str_field(o, "id") == id)
                    .map(|o| {
                        let n = if is_product {
                            str_field(o, "name")
                        } else {
                            str_field(o, "id")
                        };
                        n.to_string()
                    })
                    .unwrap_or_else(|| id.to_string())
            };
            let mut held: HashMap<&str, i64> = HashMap::new();
            for r in inventory.iter().filter(|r| r.quantity > 0) {
                let key = if is_product {
                    r.product_id.as_str()
                } else {
                    r.location_id.as_str()
                };
                *held.entry(key).or_default() += r.quantity;
            }
            for (id, qty) in held {
                if !ids.contains(id) {
                    let what = if is_product { "商品" } else { "庫位" };
                    return Err(AppError::in_use(format!(
                        "{what}「{}」尚有庫存 {qty} 件，無法刪除。請先出庫、調撥或盤點歸零後再刪除。",
                        old_name(id)
                    )));
                }
            }
            Ok(())
        }
        "remarks" => {
            // 庫存清單的「註記」：[{ product_id, location_id, remark }]
            let items = value.as_array().ok_or_else(|| AppError::invalid("資料必須是陣列"))?;
            let mut keys = HashSet::new();
            for it in items {
                let pid = str_field(it, "product_id");
                let loc = str_field(it, "location_id");
                if pid.is_empty() {
                    return Err(AppError::invalid("註記缺少品項"));
                }
                if str_field(it, "remark").chars().count() > 200 {
                    return Err(AppError::invalid("註記請在 200 個字以內"));
                }
                if !keys.insert((pid, loc)) {
                    return Err(AppError::invalid("同一品項與地點的註記重複"));
                }
            }
            Ok(())
        }
        "row_order" => {
            // 庫存清單的自訂排列順序：[{ product_id, location_id }]，依陣列先後排列
            let items = value.as_array().ok_or_else(|| AppError::invalid("資料必須是陣列"))?;
            if items.len() > 100_000 {
                return Err(AppError::invalid("排列資料過多"));
            }
            let mut keys = HashSet::new();
            for it in items {
                let pid = str_field(it, "product_id");
                if pid.is_empty() {
                    return Err(AppError::invalid("排列資料缺少品項"));
                }
                if !keys.insert((pid, str_field(it, "location_id"))) {
                    return Err(AppError::invalid("排列資料重複"));
                }
            }
            Ok(())
        }
        _ => Err(AppError::invalid(format!(
            "{stem} 不可直接寫入，庫存與日誌只能透過作業交易異動"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::storage::{create_project, project_dir, read_json};

    fn master() -> (Vec<Value>, Vec<Value>) {
        (
            vec![
                json!({"id":"PROD-001","name":"冷萃黑咖啡"}),
                json!({"id":"PROD-002","name":"綠茶"}),
            ],
            vec![json!({"id":"LOC-A-01-01"}), json!({"id":"LOC-B-01-01"})],
        )
    }

    fn run(tx: TxRequest, inv: &mut Vec<InvRow>, logs: &[Value]) -> AppResult<Vec<LogEntry>> {
        let (p, l) = master();
        compute(&tx, &p, &l, inv, logs, "2026-10-13T06:30:00Z", "20261013")
    }

    fn inbound(q: i64) -> TxRequest {
        TxRequest::Inbound {
            product_id: "PROD-001".into(),
            location_id: "LOC-A-01-01".into(),
            quantity: q,
            reason: None,
            note: None,
        }
    }
    fn outbound(q: i64) -> TxRequest {
        TxRequest::Outbound {
            product_id: "PROD-001".into(),
            location_id: "LOC-A-01-01".into(),
            quantity: q,
            reason: None,
            note: Some("#8821".into()),
        }
    }

    #[test]
    fn inbound_then_outbound() {
        let mut inv = vec![];
        let l1 = run(inbound(50), &mut inv, &[]).unwrap();
        assert_eq!(l1[0].id, "LOG-20261013-0001");
        assert_eq!(l1[0].balance_after, 50);
        let prior: Vec<Value> = l1.iter().map(|l| serde_json::to_value(l).unwrap()).collect();
        let l2 = run(outbound(10), &mut inv, &prior).unwrap();
        assert_eq!(l2[0].id, "LOG-20261013-0002");
        assert_eq!((l2[0].delta, l2[0].balance_after), (-10, 40));
        assert_eq!(qty_at(&inv, "PROD-001", "LOC-A-01-01"), 40);
    }

    #[test]
    fn outbound_more_than_stock_is_blocked() {
        let mut inv = vec![];
        run(inbound(5), &mut inv, &[]).unwrap();
        let err = run(outbound(6), &mut inv, &[]).unwrap_err();
        assert_eq!(err.code, "INSUFFICIENT_STOCK");
        assert!(err.message.contains("剩餘 5 件"));
        // 庫位沒有任何庫存時也要擋
        let err = run(
            TxRequest::Outbound {
                product_id: "PROD-002".into(),
                location_id: "LOC-A-01-01".into(),
                quantity: 1,
                reason: None,
                note: None,
            },
            &mut inv,
            &[],
        )
        .unwrap_err();
        assert_eq!(err.code, "INSUFFICIENT_STOCK");
    }

    #[test]
    fn zero_or_negative_quantity_rejected() {
        let mut inv = vec![];
        assert_eq!(run(inbound(0), &mut inv, &[]).unwrap_err().code, "INVALID_INPUT");
        assert_eq!(run(outbound(-3), &mut inv, &[]).unwrap_err().code, "INVALID_INPUT");
    }

    #[test]
    fn transfer_moves_stock_and_writes_two_logs() {
        let mut inv = vec![];
        run(inbound(10), &mut inv, &[]).unwrap();
        let tx = |q| TxRequest::Transfer {
            product_id: "PROD-001".into(),
            from_location_id: "LOC-A-01-01".into(),
            to_location_id: "LOC-B-01-01".into(),
            quantity: q,
            note: None,
        };
        assert_eq!(
            run(tx(11), &mut inv.clone(), &[]).unwrap_err().code,
            "INSUFFICIENT_STOCK"
        );
        let logs = run(tx(10), &mut inv, &[]).unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!((logs[0].delta, logs[1].delta), (-10, 10));
        // 歸零的列會被移除
        assert!(inv.iter().all(|r| r.location_id != "LOC-A-01-01"));
        assert_eq!(qty_at(&inv, "PROD-001", "LOC-B-01-01"), 10);
    }

    #[test]
    fn stocktake_creates_profit_and_loss() {
        let mut inv = vec![];
        run(inbound(10), &mut inv, &[]).unwrap();
        let tx = TxRequest::Stocktake {
            location_id: "LOC-A-01-01".into(),
            items: vec![
                StockCount {
                    product_id: "PROD-001".into(),
                    actual: 7,
                },
                StockCount {
                    product_id: "PROD-002".into(),
                    actual: 3,
                },
            ],
            note: None,
        };
        let logs = run(tx, &mut inv, &[]).unwrap();
        assert_eq!(logs[0].action_type, "ADJUST_LOSS");
        assert_eq!(logs[0].delta, -3);
        assert_eq!(logs[1].action_type, "ADJUST_PROFIT");
        assert_eq!(logs[1].balance_after, 3);
        let same = TxRequest::Stocktake {
            location_id: "LOC-A-01-01".into(),
            items: vec![StockCount {
                product_id: "PROD-001".into(),
                actual: 7,
            }],
            note: None,
        };
        assert_eq!(run(same, &mut inv, &[]).unwrap_err().code, "INVALID_INPUT");
    }

    #[test]
    fn damage_requires_reason() {
        let mut inv = vec![];
        run(inbound(10), &mut inv, &[]).unwrap();
        let mk = |r: &str, n: &str| TxRequest::Damage {
            product_id: "PROD-001".into(),
            location_id: "LOC-A-01-01".into(),
            quantity: 2,
            reason: r.into(),
            note: n.into(),
        };
        assert!(run(mk("", "王小明"), &mut inv.clone(), &[]).is_err());
        let logs = run(mk("道具受損", ""), &mut inv, &[]).unwrap();
        assert_eq!(logs[0].action_type, "DAMAGE");
        assert_eq!(qty_at(&inv, "PROD-001", "LOC-A-01-01"), 8);
    }

    #[test]
    fn free_text_and_blank_storage_location() {
        let mut inv = vec![];
        let tx = |loc: &str| TxRequest::Inbound {
            product_id: "PROD-001".into(),
            location_id: loc.into(),
            quantity: 3,
            reason: None,
            note: None,
        };
        run(tx(""), &mut inv, &[]).unwrap();
        run(tx("後倉紙箱"), &mut inv, &[]).unwrap();
        assert_eq!(qty_at(&inv, "PROD-001", ""), 3);
        assert_eq!(qty_at(&inv, "PROD-001", "後倉紙箱"), 3);
        assert!(run(tx(" 前後空白 "), &mut inv, &[]).is_err());
        assert!(run(tx(&"長".repeat(51)), &mut inv, &[]).is_err());
    }

    #[test]
    fn deserializes_frontend_payload() {
        let tx: TxRequest = serde_json::from_value(json!({
            "kind": "OUTBOUND", "product_id": "PROD-001", "location_id": "LOC-A-01-01", "quantity": 3, "note": "x"
        }))
        .unwrap();
        assert!(matches!(tx, TxRequest::Outbound { quantity: 3, .. }));
    }

    #[test]
    fn run_transaction_writes_both_files_and_survives_crash_replay() {
        let root = crate::engine::storage::tests_support::temp_root("tx");
        create_project(&root, "p").unwrap();
        let dir = project_dir(&root, "p").unwrap();
        let (p, l) = master();
        write_json_atomic(&dir, "products", &Value::Array(p), true).unwrap();
        write_json_atomic(&dir, "locations", &Value::Array(l), true).unwrap();

        let first = run_transaction(&dir, &inbound(20)).unwrap();
        assert_eq!(first.products.unwrap()[0]["first_qty"], 20);
        let second = run_transaction(&dir, &inbound(1)).unwrap();
        assert!(second.products.is_none(), "第二次入庫不可覆寫首次數量");
        run_transaction(&dir, &outbound(1)).unwrap();
        let err = run_transaction(&dir, &outbound(21)).unwrap_err();
        assert_eq!(err.code, "INSUFFICIENT_STOCK");
        // 失敗的交易不得寫入任何日誌
        assert_eq!(read_array(&dir, "logs").unwrap().len(), 3);
        run_transaction(&dir, &outbound(5)).unwrap();
        assert_eq!(read_inventory(&dir).unwrap()[0].quantity, 15);
        assert_eq!(read_array(&dir, "logs").unwrap().len(), 4);
        assert!(!file_path(&dir, PENDING_FILE).exists());

        // 模擬：日誌已寫入、庫存尚未寫入時斷電
        let mut inv = read_inventory(&dir).unwrap();
        inv[0].quantity = 12;
        let entry = json!({"id":"LOG-20991231-0001","timestamp":"x","action_type":"OUTBOUND","product_id":"PROD-001","product_name":"冷萃黑咖啡","location_id":"LOC-A-01-01","delta":-3,"balance_after":12,"reason":"r","note":""});
        let mut logs = read_array(&dir, "logs").unwrap();
        logs.push(entry.clone());
        write_json_atomic(&dir, "logs", &Value::Array(logs), true).unwrap();
        write_json_atomic(
            &dir,
            PENDING_FILE,
            &json!({"inventory": inv, "new_logs": [entry]}),
            false,
        )
        .unwrap();
        assert!(commit_pending(&dir).unwrap());
        assert_eq!(read_inventory(&dir).unwrap()[0].quantity, 12);
        assert_eq!(read_array(&dir, "logs").unwrap().len(), 5, "日誌不可重複追加");
        assert!(!commit_pending(&dir).unwrap());
        let _ = read_json(&dir, "logs").unwrap();
    }

    #[test]
    fn cannot_delete_product_or_location_with_stock() {
        let root = crate::engine::storage::tests_support::temp_root("del");
        create_project(&root, "p").unwrap();
        let dir = project_dir(&root, "p").unwrap();
        let (p, l) = master();
        write_json_atomic(&dir, "products", &Value::Array(p.clone()), true).unwrap();
        write_json_atomic(&dir, "locations", &Value::Array(l.clone()), true).unwrap();
        run_transaction(&dir, &inbound(3)).unwrap();

        let err = validate_master_write(&dir, "products", &json!([p[1]])).unwrap_err();
        assert_eq!(err.code, "IN_USE");
        assert!(err.message.contains("冷萃黑咖啡"));
        assert!(
            validate_master_write(&dir, "products", &json!([p[0]])).is_ok(),
            "沒有庫存的商品可刪除"
        );
        assert_eq!(
            validate_master_write(&dir, "locations", &json!([l[1]]))
                .unwrap_err()
                .code,
            "IN_USE"
        );
        assert!(validate_master_write(&dir, "inventory", &json!([])).is_err());
        let dup = json!([{"id":"A","name":"a","barcode":"1"},{"id":"B","name":"b","barcode":"1"}]);
        assert!(validate_master_write(&dir, "products", &dup).is_err());
    }

    #[test]
    fn row_order_validation() {
        let root = crate::engine::storage::tests_support::temp_root("order");
        create_project(&root, "p").unwrap();
        let dir = project_dir(&root, "p").unwrap();
        let ok = json!([
            {"product_id":"PROD-001","location_id":""},
            {"product_id":"PROD-001","location_id":"A 架"},
            {"product_id":"PROD-002","location_id":""}
        ]);
        assert!(validate_master_write(&dir, "row_order", &ok).is_ok());
        let dup = json!([
            {"product_id":"PROD-001","location_id":""},
            {"product_id":"PROD-001","location_id":""}
        ]);
        assert!(validate_master_write(&dir, "row_order", &dup).is_err());
        assert!(validate_master_write(&dir, "row_order", &json!([{"location_id":"A"}])).is_err());
        assert!(validate_master_write(&dir, "row_order", &json!({})).is_err());
        assert_eq!(read_json(&dir, "row_order").unwrap(), json!([]));
    }

    fn inbound_at(pid: &str, loc: &str, q: i64) -> TxRequest {
        TxRequest::Inbound {
            product_id: pid.into(),
            location_id: loc.into(),
            quantity: q,
            reason: None,
            note: None,
        }
    }
    fn remove_loc(from: &str, to: &str) -> TxRequest {
        TxRequest::RemoveLocation {
            location_id: from.into(),
            to_location_id: to.into(),
            note: None,
        }
    }

    #[test]
    fn remove_location_moves_all_stock_and_logs_pairs() {
        let mut inv = vec![];
        run(inbound_at("PROD-001", "A 櫃", 10), &mut inv, &[]).unwrap();
        run(inbound_at("PROD-002", "A 櫃", 5), &mut inv, &[]).unwrap();
        run(inbound_at("PROD-001", "B 櫃", 3), &mut inv, &[]).unwrap();
        run(inbound_at("PROD-002", "", 1), &mut inv, &[]).unwrap();

        let logs = run(remove_loc("A 櫃", "B 櫃"), &mut inv, &[]).unwrap();
        // 每個品項一出一入；被移除的地點結存歸零，新地點是加總後的數量
        assert_eq!(logs.len(), 4);
        assert!(logs.iter().all(|l| l.action_type == "TRANSFER"));
        assert_eq!((logs[0].location_id.as_str(), logs[0].delta, logs[0].balance_after), ("A 櫃", -10, 0));
        assert_eq!((logs[1].location_id.as_str(), logs[1].delta, logs[1].balance_after), ("B 櫃", 10, 13));
        assert_eq!((logs[3].location_id.as_str(), logs[3].delta, logs[3].balance_after), ("B 櫃", 5, 5));
        assert_eq!(logs[0].reason, "移除存放地點：移至 B 櫃");
        assert_eq!(logs[1].reason, "移除存放地點：由 A 櫃 移入");
        assert_eq!(logs[3].id, "LOG-20261013-0004");
        assert!(inv.iter().all(|r| r.location_id != "A 櫃"));
        assert_eq!(qty_at(&inv, "PROD-001", "B 櫃"), 13);
        assert_eq!(qty_at(&inv, "PROD-002", "B 櫃"), 5);
        // 其他地點不受影響，全部數量不變
        assert_eq!(qty_at(&inv, "PROD-002", ""), 1);
        assert_eq!(inv.iter().map(|r| r.quantity).sum::<i64>(), 19);

        // 移到「未指定」（空白）
        let logs = run(remove_loc("B 櫃", ""), &mut inv, &[]).unwrap();
        assert_eq!(logs[0].reason, "移除存放地點：移至 未指定");
        assert_eq!(qty_at(&inv, "PROD-002", ""), 6);
        assert_eq!(qty_at(&inv, "PROD-001", ""), 13);
        assert_eq!(inv.len(), 2);
    }

    #[test]
    fn remove_location_rejects_bad_requests_and_allows_empty_location() {
        let mut inv = vec![];
        run(inbound_at("PROD-001", "A 櫃", 10), &mut inv, &[]).unwrap();
        // 「未指定」不能移除、不能移到自己、目的地格式要正確
        assert_eq!(run(remove_loc("", "A 櫃"), &mut inv.clone(), &[]).unwrap_err().code, "INVALID_INPUT");
        assert_eq!(run(remove_loc("A 櫃", "A 櫃"), &mut inv.clone(), &[]).unwrap_err().code, "INVALID_INPUT");
        assert!(run(remove_loc("A 櫃", " 前後空白 "), &mut inv.clone(), &[]).is_err());
        // 加過去會超過上限：整筆中止
        let mut big = inv.clone();
        run(inbound_at("PROD-001", "B 櫃", MAX_QTY - 5), &mut big, &[]).unwrap();
        assert_eq!(run(remove_loc("A 櫃", "B 櫃"), &mut big.clone(), &[]).unwrap_err().code, "INVALID_INPUT");
        // 沒有庫存的地點：可以移除，不寫任何日誌、庫存不變
        let before = inv.clone();
        let logs = run(remove_loc("舊倉庫", ""), &mut inv, &[]).unwrap();
        assert!(logs.is_empty());
        assert_eq!(inv, before);
        // 品項已不在清單裡的舊資料也移得走（名稱以編號代替），數量為 0 的殘留列一併清掉
        let mut old = vec![
            InvRow {
                product_id: "PROD-999".into(),
                location_id: "A 櫃".into(),
                quantity: 4,
                last_updated: String::new(),
            },
            InvRow {
                product_id: "PROD-001".into(),
                location_id: "A 櫃".into(),
                quantity: 0,
                last_updated: String::new(),
            },
        ];
        let logs = run(remove_loc("A 櫃", ""), &mut old, &[]).unwrap();
        assert_eq!(logs.len(), 2);
        assert_eq!(logs[0].product_name, "PROD-999");
        assert_eq!(old.len(), 1);
        assert_eq!(qty_at(&old, "PROD-999", ""), 4);
        // 數量是負的列（被人手動改壞的資料）：不會被默默清掉，整筆交易中止
        let mut broken = vec![InvRow {
            product_id: "PROD-001".into(),
            location_id: "A 櫃".into(),
            quantity: -3,
            last_updated: String::new(),
        }];
        assert_eq!(run(remove_loc("A 櫃", ""), &mut broken, &[]).unwrap_err().code, "INSUFFICIENT_STOCK");
    }

    #[test]
    fn remove_location_payload_defaults_to_unspecified() {
        let tx: TxRequest = serde_json::from_value(json!({ "kind": "REMOVE_LOCATION", "location_id": "A 櫃" })).unwrap();
        assert!(matches!(
            tx,
            TxRequest::RemoveLocation { ref location_id, ref to_location_id, note: None }
                if location_id == "A 櫃" && to_location_id.is_empty()
        ));
    }

    #[test]
    fn remarks_and_row_order_follow_a_removed_location() {
        let remarks = vec![
            json!({"product_id":"PROD-001","location_id":"A 櫃","remark":"先出舊批"}),
            json!({"product_id":"PROD-002","location_id":"A 櫃","remark":"客戶預留"}),
            json!({"product_id":"PROD-002","location_id":"B 櫃","remark":"下層"}),
            json!({"product_id":"PROD-003","location_id":"A 櫃","remark":"相同"}),
            json!({"product_id":"PROD-003","location_id":"B 櫃","remark":"相同"}),
            json!({"product_id":"PROD-004","location_id":"A 櫃","remark":"長".repeat(150)}),
            json!({"product_id":"PROD-004","location_id":"B 櫃","remark":"久".repeat(150)}),
            json!({"product_id":"PROD-001","location_id":"","remark":"不相干"}),
        ];
        let moved = move_remarks(remarks, "A 櫃", "B 櫃");
        let get = |pid: &str, loc: &str| {
            moved
                .iter()
                .find(|r| str_field(r, "product_id") == pid && str_field(r, "location_id") == loc)
                .map(|r| str_field(r, "remark").to_string())
        };
        assert_eq!(get("PROD-001", "B 櫃").as_deref(), Some("先出舊批"), "新地點沒有註記：直接搬過去");
        assert_eq!(get("PROD-002", "B 櫃").as_deref(), Some("下層；客戶預留"), "兩邊都有：接在後面");
        assert_eq!(get("PROD-003", "B 櫃").as_deref(), Some("相同"), "內容相同不重複");
        // 接起來太長：新地點保留原本的，舊的那一則留在原處不刪
        assert_eq!(get("PROD-004", "B 櫃").as_deref(), Some("久".repeat(150).as_str()));
        assert_eq!(get("PROD-004", "A 櫃").as_deref(), Some("長".repeat(150).as_str()));
        let still_at_a: Vec<&str> = moved
            .iter()
            .filter(|r| str_field(r, "location_id") == "A 櫃")
            .map(|r| str_field(r, "product_id"))
            .collect();
        assert_eq!(still_at_a, vec!["PROD-004"], "其他註記都搬走了");
        assert_eq!(get("PROD-001", "").as_deref(), Some("不相干"));
        // 搬完仍符合註記檔的格式檢查（同一品項＋地點不重複、長度不超過上限）
        let root = crate::engine::storage::tests_support::temp_root("mvremark");
        create_project(&root, "p").unwrap();
        let dir = project_dir(&root, "p").unwrap();
        assert!(validate_master_write(&dir, "remarks", &Value::Array(moved)).is_ok());

        let order = vec![
            json!({"product_id":"PROD-001","location_id":"A 櫃"}),
            json!({"product_id":"PROD-002","location_id":"B 櫃"}),
            json!({"product_id":"PROD-002","location_id":"A 櫃"}),
            json!({"product_id":"PROD-003","location_id":""}),
        ];
        let moved = move_row_order(order, "A 櫃", "B 櫃");
        assert_eq!(
            moved,
            vec![
                json!({"product_id":"PROD-001","location_id":"B 櫃"}),
                json!({"product_id":"PROD-002","location_id":"B 櫃"}),
                json!({"product_id":"PROD-003","location_id":""}),
            ],
            "位置不變；新地點本來就有的品項不重複"
        );
        assert!(validate_master_write(&dir, "row_order", &Value::Array(moved)).is_ok());
    }

    #[test]
    fn remove_location_updates_side_files_and_survives_crash_replay() {
        let root = crate::engine::storage::tests_support::temp_root("rmloc");
        create_project(&root, "p").unwrap();
        let dir = project_dir(&root, "p").unwrap();
        let (p, _) = master();
        write_json_atomic(&dir, "products", &Value::Array(p), true).unwrap();
        run_transaction(&dir, &inbound_at("PROD-001", "A 櫃", 10)).unwrap();
        run_transaction(&dir, &inbound_at("PROD-002", "A 櫃", 4)).unwrap();
        run_transaction(&dir, &inbound_at("PROD-001", "B 櫃", 3)).unwrap();
        write_json_atomic(
            &dir,
            "remarks",
            &json!([{"product_id":"PROD-001","location_id":"A 櫃","remark":"先出舊批"}]),
            true,
        )
        .unwrap();
        write_json_atomic(
            &dir,
            "row_order",
            &json!([{"product_id":"PROD-002","location_id":"A 櫃"},{"product_id":"PROD-001","location_id":"A 櫃"}]),
            true,
        )
        .unwrap();
        let name_before = read_json(&dir, "config").unwrap()["name"].clone();

        let out = run_transaction(&dir, &remove_loc("A 櫃", "B 櫃")).unwrap();
        assert_eq!(out.new_logs.len(), 4);
        assert!(out.products.is_none());
        // 設定：記下移除時間，原有內容保留
        let config = read_json(&dir, "config").unwrap();
        assert_eq!(config["name"], name_before);
        let removed_at = config["removed_locations"]["A 櫃"].as_str().unwrap().to_string();
        assert_eq!(removed_at, out.new_logs[0].timestamp, "移除時間和這次異動的時間相同");
        assert_eq!(out.config.as_ref().unwrap(), &config);
        // 註記與排列順序跟著搬
        let remarks = read_json(&dir, "remarks").unwrap();
        assert_eq!(remarks, json!([{"product_id":"PROD-001","location_id":"B 櫃","remark":"先出舊批"}]));
        assert_eq!(out.remarks.as_ref().unwrap(), &remarks);
        let order = read_json(&dir, "row_order").unwrap();
        assert_eq!(
            order,
            json!([{"product_id":"PROD-002","location_id":"B 櫃"},{"product_id":"PROD-001","location_id":"B 櫃"}])
        );
        assert_eq!(out.row_order.as_ref().unwrap(), &order);
        // 庫存與歷史紀錄
        let inv = read_inventory(&dir).unwrap();
        assert!(inv.iter().all(|r| r.location_id == "B 櫃"));
        assert_eq!(qty_at(&inv, "PROD-001", "B 櫃"), 13);
        assert_eq!(read_array(&dir, "logs").unwrap().len(), 7);
        assert!(!file_path(&dir, PENDING_FILE).exists());
        // 一般交易不會動到這三個檔，也不回傳
        let plain = run_transaction(&dir, &inbound_at("PROD-001", "B 櫃", 1)).unwrap();
        assert!(plain.config.is_none() && plain.remarks.is_none() && plain.row_order.is_none());

        // 模擬：交易日誌寫好後斷電（庫存、設定、註記都還沒寫）→ 下次開啟時補完，重做不會重複
        let mut inv = read_inventory(&dir).unwrap();
        inv.retain(|r| r.product_id != "PROD-002");
        let mut cfg = read_json(&dir, "config").unwrap();
        cfg["removed_locations"]["C 櫃"] = json!("2099-01-01T00:00:00Z");
        write_json_atomic(
            &dir,
            PENDING_FILE,
            &json!({
                "inventory": inv,
                "new_logs": [],
                "files": { "config": cfg, "remarks": [], "row_order": "格式不對的內容會被略過", "inventory": [] }
            }),
            false,
        )
        .unwrap();
        assert!(commit_pending(&dir).unwrap());
        assert_eq!(read_json(&dir, "config").unwrap()["removed_locations"]["C 櫃"], "2099-01-01T00:00:00Z");
        assert_eq!(read_json(&dir, "remarks").unwrap(), json!([]));
        assert_eq!(read_json(&dir, "row_order").unwrap(), order, "格式不對的附帶檔不寫入");
        assert!(read_inventory(&dir).unwrap().iter().all(|r| r.product_id == "PROD-001"), "files 裡不可覆寫庫存");
        assert!(!commit_pending(&dir).unwrap());
    }
}
