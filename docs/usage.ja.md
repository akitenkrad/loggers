[English](usage.md) | **日本語**

# 使い方

## インストール

```bash
cargo add loggers
```

Rust 1.85 以降（edition 2024）が必要です．

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

## 出力形式

受理されたレコードは，1行1レコードの JSON（JSON Lines）としてログファイルに追記されます．
行単位で読み戻せる形式です．

```json
{"severity":"INFO","timestamp":"2026-08-22T15:30:00.000+09:00","target":"test","message":"Hello, world!"}
```

| フィールド | 内容 |
| --- | --- |
| `severity` | レコードのレベル．`ERROR` / `WARN` / `INFO` / `DEBUG` / `TRACE` |
| `timestamp` | ローカル時刻．RFC 3339，ミリ秒精度 |
| `target` | ロガーの名前ではなく，レコード自身の target |
| `message` | 整形済みのメッセージ |

同じレコードは人間可読な形式で stdout にも出力されます．

```text
[INFO] test 2026-08-22T15:30:00.000+09:00 - Hello, world!
```

## target によるルーティング

`Logger` はディスパッチャです．分けたい target ごとに `CustomLogger` を1つ登録すると，
各レコードは target が一致するすべてのロガーへ渡されます．

```rust
use loggers::*;

let mut logger = Logger::new();
logger.add_logger(Box::new(CustomLogger::new("api", "logs/api.log")));
logger.add_logger(Box::new(CustomLogger::new("db", "logs/db.log")));
```

target が `"api"` のレコードは `logs/api.log` だけに記録されます．
同じ target を2つのロガーが受け取ることもでき，その場合は両方が記録します．

## フォールバック

どのロガーも受理しなかったレコードは，フォールバックに渡されます．
フォールバックは，任意の target を受理する `CustomLogger::catch_all` で作ります．

```rust
use loggers::*;

let mut logger = Logger::new();
logger.add_logger(Box::new(CustomLogger::new("api", "logs/api.log")));
logger.set_fallback(Box::new(CustomLogger::catch_all("logs/everything-else.log")));
```

`CustomLogger::new` が作るのは **target で絞り込むロガー**なので，
これをフォールバックに使うと，本来受け止めるはずのレコードを自分で弾いてしまいます．
`set_fallback` は任意の `log::Log` を受け取るためコンパイルは通りますが，
フォールバックとしては機能しません．

記録される `target` はレコード自身の target なので，
catch-all なフォールバックでも各レコードの出所は分かります．

## stdout への出力を止める

レコードを受理したロガーはすべて stdout に出力するため，
2つのロガーが受理したレコードは2回表示されます．
ファイルにだけ残したい場合は，ロガー単位で止められます．

```rust
use loggers::*;

let quiet = CustomLogger::new("api", "logs/api.log").with_stdout(false);
```

ファイル出力には影響しません．

## ロガーを単体で使う

`CustomLogger` 自体が `log::Log` なので，ファイルが1つで足りる場合は
`Logger` を介さずそのまま設定できます．

```rust
use loggers::*;

log::set_boxed_logger(Box::new(CustomLogger::catch_all("logs/app.log")))
    .expect("Failed to set logger");
log::set_max_level(log::LevelFilter::Info);
```

## API リファレンス

API ドキュメントは [docs.rs](https://docs.rs/loggers) で公開しています．
