#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]
//! treasuryx 热路径：合成信封解析 + 曲线路由 / 写入主权守卫。
//!
//! `harness = false`；`--quick` 走小迭代数，便于本地快速自检。
use std::hint::black_box;
use std::time::Instant;

use treasuryx::{guard_write_authority, parse_treasury_records};

/// 合成样本（内联，避免 bench 依赖文件读取）。
const SAMPLE: &str = r#"{
    "_dataset": "DS05",
    "data": [
        {"record_date":"2026-08-18","debt_held_public_amt":29000000000000.0,
         "intragov_hold_amt":7500000000000.0,"tot_pub_debt_out_amt":36500000000000.0}
    ],
    "meta": {"count": 1, "total-count": 1, "total-pages": 1}
}"#;

fn iters() -> u32 {
    if std::env::args().any(|arg| arg == "--quick") {
        200
    } else {
        5_000
    }
}

fn main() {
    let n = iters();
    for _ in 0..n.min(10) {
        let _ = parse_treasury_records(SAMPLE).expect("合成样本可解析");
    }
    let start = Instant::now();
    let mut parsed = 0usize;
    for _ in 0..n {
        let envelope = parse_treasury_records(SAMPLE).expect("合成样本可解析");
        parsed = parsed.wrapping_add(envelope.records.len());
        black_box(guard_write_authority("WALCL").is_err());
    }
    let elapsed = start.elapsed();
    println!(
        "bench_treasuryx_parse: iters={n} total={elapsed:?} per_iter={:?} records={}",
        elapsed / n,
        black_box(parsed)
    );
}
