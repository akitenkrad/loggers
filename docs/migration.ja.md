[English](migration.md) | **日本語**

# 0.1.x からの移行

0.2.0 では 0.1.x の誤った挙動を修正しているため，そのまま差し替えられる更新ではありません．
変更点の全一覧は [changelog](../CHANGELOG.md) にあります．

## フォールバックには `catch_all` が必要

```rust
# use loggers::*;
# let mut logger = Logger::new();
// 0.1.x
logger.set_fallback(Box::new(CustomLogger::new("default", "logs/fallback.log")));

// 0.2.0
logger.set_fallback(Box::new(CustomLogger::catch_all("logs/fallback.log")));
```

0.1.x のフォールバックは，渡されたレコードに対して自分自身の target フィルタを適用していました．
そのため，target がフォールバック自身の名前とたまたま一致したレコードだけを記録し，
残りは黙って捨てていました．つまり catch-all として機能していませんでした．
`CustomLogger::catch_all` は任意の target を受理します．これがフォールバックに必要な性質です．

`set_fallback` は今も任意の `log::Log` を受け取るため，旧来の書き方もコンパイルは通ります．
ただし挙動は 0.1.x のままです．

## ログファイルが切り詰められなくなった

`CustomLogger::new` は以前 `File::create` を呼んでおり，これは既存の内容を切り詰めます．
現在は追記モードで開きます．
毎回空のファイルから始めたい場合は，ロガーを構築する前に自分でファイルを削除してください．

## `target` フィールドの意味が変わった

記録される `target` は，ロガーの名前ではなくレコード自身の target になりました．
target で絞り込むロガーでは両者は同一で，違いが出るのは catch-all なフォールバックの場合だけです．
そこではレコード自身の target の方が有用です．

ログファイルをパースして `target` で分岐している場合は，
「フォールバックのレコードがすべてフォールバックの名前を持つ」という前提に
依存していないか確認してください．

## 出力が正しい JSON になった

0.1.x は各行を `format!` で組み立てていたため，
メッセージ中の引用符・バックスラッシュ・改行がパース不能な行を生んでいました．
現在は正しくエスケープされます．
旧来の破損を回避していたパーサは簡素化できます．

## 0.2.0 での追加

- `CustomLogger::catch_all(path)` — 任意の target を受理するフォールバック．
- `CustomLogger::with_stdout(false)` — ファイルのみへの出力．
- `CustomLogger::filepath()` — そのロガーが追記するパス．
- `CustomLogger::new` と `catch_all` が `impl AsRef<Path>` を受け取るようになり，
  `&str` に加えて `PathBuf` も渡せます．
