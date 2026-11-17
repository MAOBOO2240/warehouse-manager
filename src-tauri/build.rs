use std::{env, fs, path::Path};

fn main() {
    write_update_token();
    tauri_build::build()
}

/// 和 src/update.rs 的 fnv1a 相同：還原權杖後用它確認內容一模一樣
fn fnv1a(bytes: &[u8]) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in bytes {
        h ^= u32::from(*b);
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// 私人倉庫的更新權杖（GitHub Actions 的 UPDATE_TOKEN 祕密，編譯時經由環境變數 WM_UPDATE_TOKEN 傳進來）。
/// 打散後寫進 OUT_DIR/update_token.rs 給 src/update.rs 引入：執行檔裡不會出現權杖的明文。
/// 這只是避免被直接搜尋到，不是加密——請只給「這個倉庫、Contents 唯讀」的權杖。
/// 沒有設定時產生空的權杖（倉庫公開時不需要）。
fn write_update_token() {
    println!("cargo:rerun-if-env-changed=WM_UPDATE_TOKEN");
    let token = env::var("WM_UPDATE_TOKEN").unwrap_or_default();
    let token = token.trim().as_bytes();
    // 亂數種子由權杖內容算出（同樣的權杖每次編譯結果都一樣）；xorshift 的種子不可為 0
    let mut seed: u32 = 0x9E37_79B9 ^ fnv1a(token).rotate_left(11);
    if seed == 0 {
        seed = 0x9E37_79B9;
    }
    let mut x = seed;
    let enc: Vec<u8> = token
        .iter()
        .map(|b| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            b ^ (x as u8)
        })
        .collect();
    let code = format!(
        "const TOKEN_SEED: u32 = {seed};\nconst TOKEN_SUM: u32 = {sum};\nconst TOKEN_ENC: [u8; {len}] = {enc:?};\n",
        sum = fnv1a(token),
        len = enc.len(),
    );
    let out = Path::new(&env::var("OUT_DIR").expect("OUT_DIR")).join("update_token.rs");
    fs::write(out, code).expect("write update_token.rs");
}
