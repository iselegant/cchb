# cchist 実装計画 - Claude Code セッション履歴ブラウザ

## Context

Claude Codeの過去セッション情報を閲覧・復元するためのRust製TUI CLIツールを新規作成する。
ccresume(https://github.com/sasazame/ccresume)を参考に、セッション一覧表示、会話内容プレビュー、fuzzy検索、日付フィルタリング機能を持つ。

## データソース

- **セッションファイル**: `~/.claude/projects/<dash-encoded-path>/<sessionId>.jsonl`
- **高速インデックス**: `~/.claude/projects/<path>/sessions-index.json` (存在する場合)
- **グローバル履歴**: `~/.claude/history.jsonl`
- パスエンコード: `/Users/foo/bar` → `-Users-foo-bar`
- メッセージ種別: user, assistant, system, file-history-snapshot 等
- 表示対象: userのテキスト + assistantのtextブロックのみ（thinking/tool_useはスキップ）

## 技術スタック

```toml
ratatui = "0.30"
crossterm = "0.29"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
directories = "5"
nucleo = "0.5"
anyhow = "1"
```

## UIレイアウト

```
┌─ cchist - Claude Code History ──────────────────┐
├──────────────────┬──────────────────────────────┤
│ Sessions (35%)   │ Conversation (65%)           │
│                  │                              │
│ > project-a      │ You:                         │
│   2026-04-08     │   terraform planを実行して    │
│   "terraform..." │                              │
│                  │ Claude:                       │
│   project-b      │   実行結果は以下の通りです... │
│   2026-04-07     │                              │
├──────────────────┴──────────────────────────────┤
│ 42 sessions │ f:search d:date h:help q:quit     │
└─────────────────────────────────────────────────┘
```

## キーバインド

| キー | モード | 動作 |
|------|--------|------|
| `j`/`k` | Normal/Viewing | 上下移動 |
| `g`/`G` | Normal/Viewing | 先頭/末尾 |
| `Ctrl+d`/`Ctrl+u` | Normal/Viewing | 半ページスクロール |
| `Enter`/`l` | Normal | セッション展開（Viewing） |
| `Esc`/`q` | Viewing | 一覧に戻る / Normal時は終了 |
| `Tab` | Normal/Viewing | パネル切替 |
| `f` | Normal | fuzzy検索モード |
| `d` | Normal | 日付フィルタモード |
| `c` | Normal | フィルタクリア |
| `h`/`?` | Normal | ヘルプ表示 |
| `/` | Viewing | 会話内テキスト検索 |
| `r` | Normal | 選択中セッションを復元（claude --resume） |
| `R` | Normal | セッション再読込 |
| `[`/`]` | Viewing | 前後のセッションに移動 |

## モジュール構成

```
src/
  main.rs       -- エントリポイント、ターミナル設定/復元
  app.rs        -- AppState、モード管理、状態遷移
  session.rs    -- データ型定義、JSONL解析、セッション探索
  ui.rs         -- ratatui描画（レイアウト、ウィジェット、オーバーレイ）
  event.rs      -- crossterm イベントループ、キー入力ディスパッチ
  filter.rs     -- nucleo fuzzy検索 + 日付範囲フィルタ
  color.rs      -- カラーテーマ定数
```

## 実装フェーズ（TDDベース）

### Phase 1: プロジェクト初期化
- `cargo init --name cchist`
- Cargo.toml に依存関係追加
- モジュールファイル作成（スケルトン）

### Phase 2: データ層 (`session.rs`) - テスト先行
1. テスト作成: JSONL解析、パスデコード、メッセージフィルタリング
2. 型定義: `SessionIndex`, `ConversationMessage`, `ContentBlock`
3. 実装: `discover_sessions()`, `load_conversation()`, `display_messages()`
4. sessions-index.json 高速パス + JSONL フォールバック

### Phase 3: アプリ状態 (`app.rs`) - テスト先行
1. テスト作成: 状態遷移（選択移動、モード切替、スクロール）
2. 型定義: `AppState`, `AppMode`, `Panel`
3. 実装: 各状態遷移メソッド

### Phase 4: フィルタエンジン (`filter.rs`) - テスト先行
1. テスト作成: fuzzy検索、日付フィルタ、複合フィルタ
2. 実装: nucleo統合、日付範囲フィルタリング

### Phase 5: カラーテーマ (`color.rs`)
- テーマ定数定義（テスト不要）

### Phase 6: UI描画 (`ui.rs`)
- レイアウト構築（35%/65%分割）
- セッション一覧描画
- 会話ビューア描画
- 検索/日付/ヘルプ オーバーレイ

### Phase 7: イベント処理 (`event.rs`) - テスト先行
1. テスト作成: キーディスパッチ
2. イベントループ実装
3. モード別キーハンドリング

### Phase 8: メインエントリ (`main.rs`)
- ターミナル初期化/復元（パニック時含む）
- セッション探索→ソート→状態作成→イベントループ

### Phase 9: 統合テスト・ポリッシュ
- 統合テスト（モックデータ使用）
- エッジケース対応（空ファイル、不正JSON、超長文、CJKテキスト）
- パフォーマンス最適化（遅延読込、LRUキャッシュ）

### Phase 10: バイナリ配布 & CI/CD
- Cargo.toml に release profile 最適化設定を追加
- GitHub Actions ワークフロー (`.github/workflows/release.yml`):
  - タグプッシュでトリガー
  - macOS (aarch64, x86_64) + Linux (x86_64) のクロスビルド
  - GitHub Releases にバイナリ添付
- GitHub Actions ワークフロー (`.github/workflows/ci.yml`):
  - PR/push で `cargo test`, `cargo clippy`, `cargo fmt --check` 実行

### Phase 11: セッション復元機能 (`r` キー)

#### Context
`r` キーで選択中のセッションを `claude --resume <session-id>` で復元起動する機能を追加。
既存の `r`（セッション再読込）は `R` (Shift+r) に移動。

#### 変更ファイル
1. **`src/app.rs`**: `resume_session_id: Option<String>` フィールド追加。復元要求時にセットし `should_quit = true`
2. **`src/event.rs`**:
   - `r` キー → `app.request_resume()` を呼び出し（選択中のsession_idをセット＋quit）
   - `R` キー → 既存のreload処理（main.rsで処理されるためスキップ）
3. **`src/main.rs`**:
   - `run_app` のreloadを `R` (Shift+r) に変更
   - ループ終了後、`app.resume_session_id` が Some なら `std::process::Command::new("claude").args(["--resume", &id])` で起動
4. **`src/ui.rs`**: ステータスバーのヒントに `r:resume` を追加
5. **テスト**: `event.rs` に `test_r_requests_resume`, `test_shift_r_does_not_resume` を追加

#### 検証
- `r` を押すとTUIが終了し `claude --resume <session-id>` が実行される
- `R` を押すとセッション一覧が再読込される

## 検証手順

各フェーズ完了後:
- `cargo test` 全テストパス
- `cargo clippy` 警告なし
- `cargo fmt --check` パス

最終検証:
- `cargo run` でセッション一覧が表示される
- j/kでナビゲーション動作
- Enterでセッション展開、会話が右パネルに表示
- fキーでfuzzy検索が機能
- dキーで日付フィルタが機能
- hキーでヘルプオーバーレイ表示
- q/Ctrl+Cでターミナルが正常復元
