<p align="center"><img src="docs/assets/hero.svg" width="100%"></p>

[English](README.md) | **日本語**

[![CI](https://github.com/akitenkrad/rs-loggers/actions/workflows/ci.yml/badge.svg)](https://github.com/akitenkrad/rs-loggers/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/loggers.svg)](https://crates.io/crates/loggers)
[![docs.rs](https://docs.rs/loggers/badge.svg)](https://docs.rs/loggers)

# loggers

各レコードを target に応じたログファイルへ振り分ける，小さな
[`log`](https://crates.io/crates/log) バックエンドです．
どのロガーも受理しなかったレコードを受け止める catch-all なフォールバックを設定できます．
レコードは JSON Lines として追記され，人間可読な形式で stdout にも出力されます．

## インストール

```bash
cargo add loggers
```

## クイックスタート

```rust
use log::{debug, info};
use loggers::*;

let mut logger = Logger::new();

// target が "test" のレコードは system.log へ．
logger.add_logger(Box::new(CustomLogger::new("test", "tests/output/system.log")));

// それ以外はすべて fallback.log へ．
logger.set_fallback(Box::new(CustomLogger::catch_all("tests/output/fallback.log")));

log::set_boxed_logger(Box::new(logger)).expect("Failed to set logger");
log::set_max_level(log::LevelFilter::Trace);

info!(target: "test", "Hello, world!");
debug!("Default");
```

## ドキュメント

- [使い方](docs/usage.ja.md) — target によるルーティング，フォールバック，
  出力形式，stdout 出力の停止
- [挙動と保証](docs/behaviour.ja.md) — 何を保証し，その保証がどこで途切れるか
- [0.1.x からの移行](docs/migration.ja.md) — 0.2.0 での挙動変更
- [Changelog](CHANGELOG.md)
- [API リファレンス](https://docs.rs/loggers)（docs.rs）

## ライセンス

Apache-2.0
