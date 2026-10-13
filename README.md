# 零售倉儲管理系統（單機離線版）

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
| 專案管理 | 頂部切換多個倉庫專案、新增專案、一鍵匯出 ZIP 備份；記住上次開啟的專案 |
| 商品主檔 | 新增／編輯／刪除、名稱／條碼／SKU／分類搜尋；條碼與 SKU 不可重複；有庫存的商品不可刪除 |
| 庫位主檔 | 倉區／貨架／層號，自動產生代碼（例：A區 01 01 → `LOC-A-01-01`）；有庫存的庫位不可刪除 |
| 入庫 | 掃描條碼 → 自動帶入常用庫位 → 數量 → Enter 送出 |
| 出庫 | 列出各庫位現存量；數量超過庫位庫存時按鈕停用並顯示紅字警示，Rust 端再次校驗 |
| 庫位調撥 | 來源庫位庫存不足即阻擋；兩端同時更新，流水帳寫入一出一入兩筆 |
| 盤點平倉 | 選庫位 → 掃描定位品項 → 輸入實盤數 → 即時試算盤盈／盤虧 → 確認後校正帳面 |
| 損壞與報廢 | 分為「物品損壞」(`DAMAGE`) 與「報廢損耗」(`SCRAP`)，原因與責任備註必填 |
| 儀表板 | SKU 數、在庫總件數、庫存總成本、預期零售總值；低庫存卡片可直接跳轉入庫 |
| 庫存清冊／流水帳 | 篩選（類型、起訖日期、商品名稱或條碼）並匯出 UTF-8 BOM CSV，Excel 開啟不亂碼 |

**掃描槍**：不做背景監聽。游標停在黃色外框的輸入框時掃入條碼，掃描槍送出的 Enter 會直接比對商品，
游標自動跳到數量欄；數量輸入後再按 Enter 即送出，接著游標回到掃描框，可連續作業。

**低庫存規則**：商品設有「最低安全庫存」（大於 0），且全庫總量 ≤ 該數值時列入預警。

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
* **負庫存**：前端即時阻擋；後端計算後若任何庫位會小於 0，整筆交易中止、不寫入任何檔案與日誌。
* **庫存與流水帳只能經由作業功能異動**：通用寫入 API 只接受商品、庫位、設定三種主檔。
* **損壞偵測與還原**：開啟專案時若有檔案損壞或被鎖定，系統進入唯讀並詢問
  「是否從最近一次 .bak 備份還原？」。還原時會略過同樣損壞的備份，原本損壞的檔案另存為 `*_corrupt_*.json`。

## 五、乾淨電腦驗收步驟

1. 在未安裝 Node.js／Rust 的 Windows 10/11 電腦，把 `WarehouseManager.exe` 放到桌面新資料夾，雙擊開啟。
2. 確認 exe 旁自動出現 `projects\預設總倉\`，內含 5 個 JSON 與 `backups\`。
3. 建立 2 個庫位、1 個商品（最低安全庫存設 20），入庫 50 件。
4. 出庫輸入 60 件：按鈕應停用並顯示「庫存不足！當前庫位剩餘 50 件，不可出庫 60 件」。
5. 出庫 35 件 → 儀表板出現低庫存預警（剩 15 ≤ 20），點「直接入庫」會帶入該商品。
6. 調撥 5 件到另一庫位、盤點把某庫位改成實盤數、登記 1 件物品損壞，確認流水帳各類型紀錄正確。
7. 匯出庫存清冊與流水帳 CSV，用 Excel 開啟確認中文正常。
8. 關閉程式，用記事本把 `inventory.json` 內容清空並存檔，重新開啟：應跳出還原詢問，還原後資料恢復。

## 六、與原始規格的差異

* 「供應商退貨（進貨退出）」依需求移除，改為「物品損壞」異動類型 `DAMAGE`。
* 規格中的 `projects/default/` 統一為 `projects/預設總倉/`，並額外初始化 `config.json`（共 5 個資料檔）。
* 調撥因日誌只有單一庫位欄位，寫成兩筆 `TRANSFER`（來源負數、目的正數）。
* 備份檔名加上毫秒（`inventory_20261013_140000_123.bak`），避免同一秒內多次寫入互相覆蓋。
* 開發者工具：Release 版預設即關閉（未啟用 Tauri 的 `devtools` 功能），不需另外設定。
