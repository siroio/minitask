# minitask

ユーザー承認済み: 中央の保存先、workspace別のMarkdown、複数行メモ、Neovim / Emacs編集、増分インデックス。

- 保存先: `MINITASK_HOME`、未設定ならユーザー設定ディレクトリ下の `minitask`。起動ディレクトリには依存しない。`--home`でも指定可能。
- `<保存先>/<workspace>/tasks.md` が正本。現在の形式・日付・カレンダーは `2026-09-08-obsidian-calendar-design.md` に従う。旧 `## [ ]` 形式も読み込み可能。
- 左にworkspace、右にタスクと詳細。Tabでペイン移動、j/k・矢印で選択、Spaceで完了切替、aで追加、nでworkspace作成、eでエディタ、/で検索、qで終了。
- エディタは `--editor` → `VISUAL` → `EDITOR` → `nvim`。Neovim / Emacsには実行ファイル・行番号・絶対パスを引数として直接渡す。終了後は対象ファイルを必ず再索引化する。
- `.index.json` にworkspace別のファイル更新時刻・サイズ・ハッシュ・解析結果を保存。起動時および再読み込み時は変更ファイルのみ解析。操作中はメモリ上の索引を使用。
- 検索は索引内のタイトル・詳細を対象とする部分一致。現在のworkspaceを検索する。
- `.state.json` に最後のworkspaceを保存。キャッシュと状態は消してもタスクに影響しない。
- タスク操作は最新のファイルを確認し、外部変更があれば古い選択による更新を拒否して再読み込み。書き込みは同一ディレクトリの一時ファイルから置換し、元の改行・メモを保持。
- Windowsを実機ビルド対象とし、Rust + Ratatui + Crosstermで端末処理を行う。インデックスにDBサービスは不要。
- 検証: 保存と外部変更の統合テスト、Ratatui TestBackendの操作テスト、Neovim / Emacs実プロセス、索引のcold/warmベンチマーク、Windowsビルド。

- ユーザー承認済みのYazi起動キーはCtrl+T。既存のTODO編集割当を置き換える。
