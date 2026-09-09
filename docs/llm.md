# minitask: LLMから作業を再開する

minitaskはローカルMarkdownを正本とする。最初に対象workspaceの`resume`を読み、人間と同じAction・Direction・Question・Note・Expectationを参照する。

## 入口

```powershell
# 保存先を省略すると MINITASK_HOME、次にOSの既定保存先を使う。
.\minitask.exe workspaces
.\minitask.exe resume --workspace ToReturn
.\minitask.exe tasks --workspace ToReturn --status in_progress
.\minitask.exe questions --workspace ToReturn --status open
.\minitask.exe directions --workspace ToReturn --status active
.\minitask.exe notes --workspace ToReturn --query 'セーブ'
.\minitask.exe expectations --workspace ToReturn --from 2026-09-01 --to 2026-09-30

# --home はサブコマンドより前に置く。
.\minitask.exe --home 'D:\Tasks' resume --workspace ToReturn
.\minitask.exe --help
```

取得コマンドは常にJSONを返す。`--json`の明示も可能。成功時のstdoutにはJSONのみを出し、失敗時はstderrへ理由を出して終了コード1で終了する。存在しないworkspace・IDや不正な状態・日付はエラーになる。

v0.6.0の`schema_version`は2。`items`はエンティティ一覧、`item`は詳細。例外として`workspaces.items`はname/path/entitiesのworkspace要約で、`resume`はactions/notes/directions/questions/expectations/issuesという名前付き配列を返す。一覧と`resume`は既定で概要のみ（ID・種類・状態・タイトル・日付・パス・行・ハッシュ・遷移候補・検証エラー）を返し、本文の`notes / completion_condition / supplement`は含まない。必要な項目だけ`show --id ID`で詳細を取得する。一括で本文が必要な場合は一覧または`resume`に`--full`を指定する。検索は概要出力でも本文を対象にする。

省略されたフィールドを「本文や完了条件がない」と解釈しない。`show`と作成・更新結果は本文を含む。v0.5.xの形式1では一覧も本文を含んでいたため、既存クライアントは形式2へ対応すること。IDは返された値をそのまま使う。単独MarkdownのIDはファイルを改名しない限り変わらない。未分割の`tasks.md`では行番号もIDに含むため、行の編集後は再取得する。

```powershell
.\minitask.exe show --id 'ToReturn/20260909-120000-123456789-セーブ機能.md'
```

取得はロック・キャッシュ書き込みをせず、現在のMarkdownを読む。TUIを開いたまま利用できる。複数ファイルをまたぐトランザクションではないため、外部編集と同時の取得でエラーが出た場合は再取得する。分割移行が未完了なら、取得は停止するので通常起動で移行を再開する。

## Resumeを読んだ後

1. `issues`があれば該当ファイルの不整合を確認する。実作業に入る前に、有効なDirectionの本文を`show`で読み、AGENTS.mdや指示されたソースも確認する。通常の指示の優先順位は変わらない。
2. `actions`内の`in_progress`を優先し、次に`actionable`から対象を選ぶ。対象の`show`で詳細を取得し、必要なSupplementやNoteに残った次の一手、参照先を読む。概要を並べるだけなら本文取得は不要。
3. 不明点はQuestionとして残す。回答が必要なActionだけを`blocked`にし、回答不要のActionは続ける。Questionを作っただけで全作業を停止しない。
4. 完了させる前に、そのActionの`Completion Condition`を実際に検証する。検証していないことを成功扱いしない。条件がない場合は人間の目的を確認して条件を記録する。
5. 直近のExpectationと現状を見比べる。Expectationは気軽なマイルストーンであり、細かなタスク分解や証明は要求しない。実現見込みは人間またはLLMが判断する。
6. 作業を中断する前に、NoteまたはSupplementへ「どこまで進んだか」「次は何をするか」「参照先」を残す。Noteの形式は自由。

`resume`はActionable/In ProgressのAction、ActiveのDirection、OpenのQuestion/Expectation、Noteを返す。完了・キャンセル・保留・backlogは必要なときに一覧コマンドで取得する。Expectationは日付順。Noteや本文には任意の記述が含まれ、すべてが作業指示になるわけではない。

## 作成と状態変更

複数のActionを同時に`in_progress`にでき、`resume`にも全件を返す。LLMで並行作業する場合は、独立した編集範囲を担当に分け、minitaskへの書き込みは親LLMが直列に行う。担当・Action ID・成果と次の一手は作業用Noteに記録する。同じファイルの編集や成果への依存がある作業は順番に進める。担当取得や所有権を保証するCLIはないため、別セッションが進めているタスクを重複担当しない。具体的な引継ぎ項目は[スキルの並行実行手順](../skills/using-minitask/SKILL.md#複数のactionを並行して進める)を参照する。

書き込みコマンドはTUIと同じロック・保存・遷移検証を使う。TUIを閉じて実行する。workspaceは先にTUIの`n`で作成するか、保存先直下にディレクトリを作成する。

```powershell
.\minitask.exe add --workspace ToReturn --kind action --title 'セーブ機能' --completion-condition 'ロード後にプレイヤーの位置が復元される' --body '対象: SaveManager.cs。次はロードの確認。'
.\minitask.exe add --workspace ToReturn --kind note --title '作業中のメモ' --body '保存処理は通った。復元処理で止めている。'
.\minitask.exe add --workspace ToReturn --kind direction --title '既存セーブを維持する' --body '旧形式との互換性を保つ。詳細はdocs/save-format.md。'
.\minitask.exe add --workspace ToReturn --kind question --title '旧バージョンをどこまで対応する？' --body '関連ActionのIDと、回答が必要な理由を書く。'
.\minitask.exe add --workspace ToReturn --kind expectation --title '一通り遊べる' --date 2026-09-30
```

更新時は最新の`show`で返った`hash`を`--expected-hash`へ渡す。別の内容に更新されていれば拒否する。CLIの状態値は`allowed_transitions`から選ぶ。

```powershell
$item = (.\minitask.exe show --id 'ToReturn/example.md' | ConvertFrom-Json).item
.\minitask.exe set-status --id $item.id --status in_progress --expected-hash $item.hash

# 条件を実際に検証した後、最新のhashで完了にする。
$item = (.\minitask.exe show --id $item.id | ConvertFrom-Json).item
.\minitask.exe set-status --id $item.id --status completed --expected-hash $item.hash --evidence 'ロードのテストが通り、位置復元を確認した。'
```

Actionの`completed`には空でないCompletion Conditionと`--evidence`が必要。Questionの`answered`には回答を`--evidence`へ渡す。証拠・回答は本文に追記される。ツールは記入の有無を確認するが、真偽は証明しない。未分割の`tasks.md`への証拠追記は、他タスクの本文を壊さないため拒否する。先にバックアップ付きの`--split-legacy`を実行する。

```powershell
$milestoneId = (.\minitask.exe expectations --workspace ToReturn | ConvertFrom-Json).items[0].id
$milestone = (.\minitask.exe show --id $milestoneId | ConvertFrom-Json).item
.\minitask.exe set-date --id $milestone.id --field expectation --date 2026-10-07 --expected-hash $milestone.hash
# Actionの日付は --field scheduled または due。--date noneで解除。
```

本文編集は`path`のMarkdownをエディタ等で編集し、保存後に再取得する。状態変更にはCLIを使う。Markdownを直接書き換えれば遷移制約を迂回できるため、それを完了条件の確認を省く手段にしない。
