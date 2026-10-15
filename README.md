# 倉儲管理系統（單機離線版）

Windows 單一免安裝 `.exe`，100% 本機離線運行，資料以 JSON 存在 `.exe` 旁的 `projects/` 資料夾。
技術：Tauri 2（Rust）＋ Vue 3 ＋ Vite ＋ Pinia ＋ Tailwind CSS 4。

---

## 一、取得 WarehouseManager.exe（用 GitHub 自動打包，不必在電腦上裝開發工具）

1. 到 GitHub 建立一個新的 repository（建議設為 Private）。
2. 把本資料夾的所有檔案上傳到該 repository（網頁上「Add file → Upload files」直接拖曳整個資料夾即可，
   注意 `.github` 資料夾也要一起上傳）。
3. 上傳後打開 repository 的 **Actions** 分頁，會看到「Build Windows EXE」開始執行（約 10–15 分鐘）。
   若沒有自動執行，點進「Build Windows EXE」→「Run workflow」。
4. 執行完成（綠色勾勾）後點進該次執行，頁面最下方 **Artifacts** 區的 `WarehouseManager-windows`
   就是打包好的檔案，下載解壓縮即得到 `WarehouseManager.exe`。
5. 若要發佈正式版本：建立 `v1.0.0` 這類標籤（tag），會自動建立 Release 並附上 exe。

流程中會先執行 Rust 單元測試（原子寫入、備份輪轉與還原、負庫存阻擋、斷電後交易補完），測試失敗就不會產出 exe。

### 也可以在自己的 Windows 電腦編譯

需要：Node.js 20 以上、Rust（rustup）、Visual Studio Build Tools（勾選「使用 C++ 的桌面開發」）。

```powershell
pip install pillow
python scripts/make_icons.py     # 產生程式圖示（只需執行一次）
npm install
npm run tauri dev                # 開發模式（會開啟視窗，修改程式即時更新）
npx tauri build --no-bundle      # 產出 src-tauri\target\release\warehouse-manager.exe
cargo test --manifest-path src-tauri/Cargo.toml   # 執行 Rust 單元測試（需先 npm run build）
```

---

## 二、使用前須知

* **放置位置**：請把 exe 放在可寫入的資料夾，例如桌面、`D:\倉儲\`、隨身碟。
  不要放在 `C:\Program Files`（Windows 不允許一般程式在那裡寫檔，程式會提示無法啟動）。
* **WebView2**：Windows 11 與絕大多數 Windows 10 已內建。若開啟後一片空白或提示缺少元件，
  到微軟官網下載安裝「Microsoft Edge WebView2 Runtime（Evergreen）」即可。
* **搬移到別台電腦**：把 exe 與 `projects` 資料夾一起複製過去，或用「匯出專案備份 (ZIP)」後解壓到新電腦 exe 旁的 `projects\`。
* **不要用 Excel 直接開啟 JSON 檔**：檔案被鎖住時程式會提示「檔案正被其他程式使用」。需要報表請用畫面上的「匯出 CSV」。

## 三、功能一覽

| 模組 | 說明 |
|---|---|
| 專案管理 | 頂部切換多個倉庫專案、新增／刪除專案（刪除前二次確認，資料夾移到 `projects\_已刪除專案` 可救回）、一鍵匯出 ZIP 備份 |
| 儀表板 | 有庫存品項數、在庫總件數、今日入庫、今日出貨／損壞；最近 10 筆異動 |
| 入庫 | 點品項欄列出所有品項（可捲動），或直接輸入新名稱「新增品項」；存放地點可不填；數量自動帶入該品項第一次入庫的數量 |
| 出庫/損壞 | 選品項 → 選存放地點（顯示各地點現量）→ 動作「出貨」或「損壞」；損壞需選原因：道具受損／印刷錯誤／其他（自行輸入）；數量超過現量時按鈕停用並顯示紅字，Rust 端再次校驗 |
| 庫存清單 | 大字顯示各品項在各存放地點的數量；每列可直接「入庫／出庫／損壞」，「新增地點」可把部分數量移到新地點（流水帳記一出一入）；「註記」欄點一下即可編輯；按住表頭左右拖曳可移動顯示範圍；按住列左側六點圖示（或長按該列）可拖曳排序，順序存在 row_order.json，「恢復名稱排序」可還原；可匯出 CSV |
| 異動流水帳 | 依類型（入庫／出貨／損壞／移動地點）、起訖日期（年月日自動跳格＋彈窗日曆）、品項或地點篩選，可匯出 CSV（UTF-8 BOM，Excel 開啟不亂碼） |

* 品項不需預先建檔；第一次入庫的數量會記在品項上（`first_qty`），之後入庫、出庫選到該品項時自動帶入，仍可修改。
* 存放地點為自由輸入，不需預先設定；空白會顯示為「未指定」。輸入過的地點會出現在下拉建議中。
* 介面為圓潤氣泡風格：懸浮卡片＋膠囊按鈕，卡片圓角使用 G2 連續曲率（CSS `corner-shape: squircle`，需 WebView2 139 以上）。
* 左下角齒輪「設定」：文字大小（大／中／小）與深色／淺色外觀，設定會記住。

## 四、資料安全設計

```
WarehouseManager.exe
projects/
├── _app_state.json        記錄上次開啟的專案
└── 預設總倉/
    ├── config.json  products.json  locations.json  inventory.json  logs.json
    └── backups/           每個檔案各保留最新 5 份 .bak
```

* **原子寫入**：舊檔備份到 `backups/` → 新內容寫入 `.tmp` 並 `sync_all()` → `rename` 覆蓋正本。
  任何時間點斷電，正本只會是完整舊版或完整新版，不會變成 0 位元組。
* **寫入佇列**：Rust 端所有檔案操作排隊依序執行，快速連點不會互相衝突。
* **交易一致性**：每筆庫存異動先寫入 `pending_tx.json`（交易日誌），再依序更新 `logs.json`、`inventory.json`。
  若中途斷電，下次開啟專案會自動補完，庫存與流水帳保持一致。
* **負庫存**：前端即時阻擋；後端計算後若任何存放地點會小於 0，整筆交易中止、不寫入任何檔案與日誌。
* **庫存與流水帳只能經由作業功能異動**：通用寫入 API 只接受品項清單與設定檔。
* **損壞偵測與還原**：開啟專案時若有檔案損壞或被鎖定，系統進入唯讀並詢問
  「是否從最近一次 .bak 備份還原？」。還原時會略過同樣損壞的備份，原本損壞的檔案另存為 `*_corrupt_*.json`。

## 五、乾淨電腦驗收步驟

1. 在未安裝 Node.js／Rust 的 Windows 10/11 電腦，把 `WarehouseManager.exe` 放到桌面新資料夾，雙擊開啟。
2. 確認 exe 旁自動出現 `projects\預設總倉\`，內含 JSON 資料檔與 `backups\`。
3. 入庫：品項欄輸入「測試海報」→ 選「新增品項」→ 存放地點留空 → 數量 50 → 確認入庫。
4. 再次入庫選「測試海報」：數量應自動帶入 50。
5. 出庫/損壞：選「測試海報」→ 數量應自動帶入 50 → 改成 60：按鈕停用並顯示「庫存不足！此存放地點剩餘 50 件」。
6. 動作選「出貨」出 10 件；再選「損壞」→「其他」→ 輸入原因 → 登記 2 件。流水帳應有入庫、出貨、損壞各一筆。
7. 匯出庫存清單與流水帳 CSV，用 Excel 開啟確認中文正常。
8. 關閉程式，用記事本把 `inventory.json` 內容清空並存檔，重新開啟：應跳出還原詢問，還原後資料恢復。

## 六、與原始規格的差異

* 依需求移除：條碼／SKU／掃描槍相關功能、商品主檔與庫位主檔頁面、庫位調撥、盤點平倉、報廢、入庫原因、成本與售價、低庫存預警。
* 「供應商退貨」移除，改為出庫頁的「損壞」動作（流水帳類型 `DAMAGE`）。
* 規格中的 `projects/default/` 統一為 `projects/預設總倉/`，並額外初始化 `config.json`。
* 備份檔名加上毫秒（`inventory_20261013_140000_123.bak`），避免同一秒內多次寫入互相覆蓋。
* 後端仍保留調撥、盤點等交易類型與舊資料的顯示，舊專案資料可直接開啟。
