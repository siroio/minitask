# minitask

作業の続きを思い出すためのタスク管理ツールです。ワークスペースごとにディレクトリを分け、1項目1Markdownでタスク・自由ノート・方針・質問・マイルストーンを保存します。

このZIPはWindows x64用です。RustやPythonのインストールは不要です。Emacs・Neovim・Yaziは、利用するものを別途用意してください。

## はじめる

1. ZIPを好きな場所へ展開します。以下では `C:\Tools\minitask` に置いたものとします。
2. Windows Terminalなどで、そのフォルダを開きます。
3. `./minitask.exe` で起動し、`n` でワークスペース、`a` でタスクを作成します。

`?` で操作一覧、`c` で月間カレンダー、`e` でMarkdown編集、`q` で終了します。予定日・期限は `s` / `d`、マイルストーンの作成は `m` です。

`Tab` で「作業 → ワークスペース → タスク一覧」を移動し、`Shift+Tab` で逆順に戻れます。各領域では上下矢印または `j` / `k` で選択し、`Enter` で選択を適用してタスク一覧へ進みます。

TUI・Emacsで新規タスクを追加すると、予定日・期限ともに今日になります。カレンダーから追加した場合は選択した日が入ります。TUIでは追加フォームの日付欄を変更・消去できます。Emacsでは追加後に `d` で日付を変更・解除できます。

編集に使うアプリは、起動時に `./minitask.exe --editor nvim`、または `./minitask.exe --editor 'emacs -nw'` と指定します。エディタの実行ファイルをPATHから起動できるようにしてください。省略時は `VISUAL`、`EDITOR` 環境変数、`nvim` の順で選びます。

保存先は既定で `%APPDATA%\minitask`。実行ファイルの置き場所とは別です。変更する場合は `MINITASK_HOME` 環境変数、または `./minitask.exe --home 'D:\Tasks'` を使います。ワークスペースとMarkdownはこの保存先に集約されます。

## Emacs

Emacs 27.1以降で、`init.el` に実際の展開先を指定します。

```elisp
(add-to-list 'load-path "C:/Tools/minitask/emacs")
(autoload 'minitask "minitask" nil t)
(setq minitask-executable "C:/Tools/minitask/minitask.exe")
```

初回にまだ保存先がなければ、TUIを一度起動して終了してください。その後 `M-x minitask` で開けます。`w` でワークスペース切替、`a` で追加、`e` で本文編集、`s` で状態変更、`d` で日付設定、`c` でカレンダーです。

追加パッケージは不要です。詳しくは [Emacsの使い方](docs/emacs.md) を参照してください。

ノートを `e` で開き、文章を範囲選択して `C-c C-t` を押すと、その文章からタスクを作成できます。タイトルと完了条件を入力すると、同じワークスペースに新規タスクが開きます。元の文章はノートに残ります。

## YaziからCtrl+Tで起動

展開先をPATHに追加し、同じ端末で `minitask --help` が開くことを確認します。同梱の `yazi-keymap.toml` の内容を、Yaziの `keymap.toml` に追記してください。Windowsの通常の場所は `%APPDATA%\yazi\config\keymap.toml` です。既にCtrl+Tの設定があれば、その項目を置き換えます。

設定後は端末とYaziを起動し直してください。既存の `keymap.toml` 全体を上書きする必要はありません。

## LLMから使う

展開先をPATHに追加し、`skills/using-minitask` ディレクトリを使用するエージェントのスキル保存先へコピーします。Codexでは通常 `~/.codex/skills/using-minitask/SKILL.md` となるように配置し、カスタムの `CODEX_HOME` を使う場合はその下の `skills` に置きます。同名スキルがあれば内容を確認して更新してください。

PATHを設定しない場合は、LLMへ実行ファイルの絶対パスを伝えます。保存先と対象ワークスペースも伝えてください。

「開発ワークスペースの独立したタスクを並行して進めて」と依頼できます。サブエージェント対応環境では複数のActionを同時に進行中として分担し、親LLMが管理記録の更新と完了確認を行います。担当・進捗は作業用Noteに残します。

```powershell
./minitask.exe workspaces
./minitask.exe resume --workspace '開発'
./minitask.exe tasks --workspace '開発'
./minitask.exe --help
```

引数なしではTUI、コマンド指定ではJSONを返すCLIになります。v0.6.0ではJSON形式2を使い、一覧と `resume` は本文を省いた概要を返します。必要な項目だけ `show --id ID` で詳細を取得してください。一括で本文が必要なら `tasks --full` などを指定できます。詳しい取得・更新方法と完了条件の検証手順は [LLM用スキル](skills/using-minitask/SKILL.md) にあります。CLIとEmacsから更新する間はTUIを閉じてください。更新時は本体・Emacsフロントエンド・スキルを同じZIPのものに揃えてください。

## 表示文言と更新

TUIの文言は `locales/ja.json`、Emacsの文言は `emacs/locales/ja.json` にあります。翻訳ファイルを追加すれば別の言語に切り替えられます。TUIでは `--locale 言語名`、Emacsでは `minitask-locale` を指定します。差し込み記号を保持し、Markdownの管理見出し・JSONのキー・状態値は翻訳しないでください。

更新時はアプリを閉じ、新しいZIPを別フォルダへ展開して、PATHとEmacs設定の参照先を切り替えます。タスクの保存先は引き続き同じものを使います。編集した翻訳ファイルは別途バックアップしてください。

## ライセンス

minitaskはMITライセンスです。[LICENSE](LICENSE) を参照してください。依存ライブラリの表示・ライセンス本文は `THIRD-PARTY-NOTICES.txt` と `licenses/` に同梱しています。
