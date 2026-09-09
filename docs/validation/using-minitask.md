# using-minitask 検証記録

対象: minitask 0.5.1、`skills/using-minitask/SKILL.md`。2026-09-09。
`writing-skills`のRED → GREEN → REFACTORで作成。実データの代わりに`.cache/skill-validation`配下の独立した保存先を使用する。

## 作成前の観測（RED）

| シナリオ | 観測 |
| --- | --- |
| CLI参照なしで6操作の構文・制約を回答 | baseline_cli_referenceは「minitask v0.5.1のCLI仕様を把握していないため、実行可能な具体的コマンドは提示できません」。未確認の構文を捏造しなかったが、操作を案内できないという参照情報の不足を確認。 |
| 作業再開、回答待ち、9月末の見通しを読み取りのみで確認 | baseline_resumeは成功。resumeの後にtasksを取得し、blocked・不正日付も報告。更新なし。ここに追加の禁止事項は不要。 |
| 自動チェック成功・目視未実施・時間制約の下で作業を続行 | baseline_writeは部分的な成功だけで完了せず、回答不要の案内文を進めた。一方、CLIへのOSアクセス拒否をPowerShellのパイプが隠し、当初「draft-result.jsonの書き込みは成功」と報告。後に「シェルexit_codeは0、結果nullでdraft-result.jsonは作成されていません」と訂正。 |

アクセス拒否そのものは環境の制約であり、製品・スキルの失敗とは採点しない。CLI失敗直後の終了コード確認を参照例に含める根拠とした。

## 合格条件

1. 保存先とworkspaceを限定し、JSONの正しい配列とIDを使える。
2. blockedがresumeにないことを踏まえ、必要な一覧を追加取得できる。
3. Actionの条件を検証し、未実施の目視確認や人間の回答を捏造しない。
4. 質問待ちでも独立作業を進め、自由Noteと軽いExpectationを扱える。
5. 更新は最新内容とhashを使い、競合時に条件を再評価する。ロックを迂回しない。
6. 日本語の内容と英語の管理項目を区別し、未実装CLIを発明しない。
7. CLIエラーを空JSONや更新成功にしない。

## チェックリスト

- [x] 利用・圧力シナリオを作成
- [x] スキルなしの基準動作を実行
- [x] 正確な失敗・不足と既に正しかった判断を記録
- [x] 名前を英小文字・ハイフンで作成
- [x] name / descriptionのfrontmatterを作成
- [x] descriptionをUse whenで開始し、利用条件のみ記述
- [x] descriptionは第三者視点
- [x] minitask・CLI・workspace等の発見用語を含める
- [x] 概要と用途を明示
- [x] REDで観測したCLI参照不足と終了コードの落とし穴に対応
- [x] 参照不足には構文表、終了コードには実行例を使う
- [x] 文言誘導の5反復マイクロテストは対象外（API参照スキル。行動の出力形式の誘導ではない）
- [x] コードは本文に記載
- [x] PowerShellの更新例を掲載
- [x] スキルありの独立エージェントで同じシナリオを実行
- [x] 新たな誤解・回避行動を確認
- [x] 必要な修正と再試験
- [x] 合理化の表は対象外（規律違反の合理化は未観測）
- [x] 注意点に競合・部分成功・未確認の完了を明記
- [x] 不要なフローチャートなし
- [x] 操作早見表と間違えやすい点あり
- [x] スキル本文は経緯の物語にしない
- [x] 配布スキルに補助ファイル・ランタイム依存を追加しない
- [x] frontmatter検証と実CLIの例・異常系を確認
- [x] 個人スキルへ配置し、内容一致を確認
- [x] コミット・push・外部公開は今回の依頼範囲外

## 実行結果

| 検証 | 結果 |
| --- | --- |
| green_cli_reference（元の6問＋期限圧力・条件変更・TUIロック） | 6操作を正しく案内。未実装の本文編集／workspace作成CLIを発明せず、条件変更時の再検証とロック解放を選択。 |
| green_resume | workspaceを限定してworkspaces / resume / tasksを取得。完了状況と回答待ちを区別。元の保存先と全ファイルのバイト一致、キャッシュ・状態ファイルなしを確認。 |
| green_write | 未実施のプレビューは未完了を維持。独立した案内文をNoteに保存し、再取得で確認してActionをcompletedに変更。既存自由Noteへ追記し、元の本文を保持。質問・保留・Expectation・別案件は維持。 |
| OSアクセス拒否への対応 | GREENでは直後に失敗を検出。再取得で未保存を確かめ、許可された隔離領域への昇格実行で続行。ロック回避なし。 |
| 実装との独立レビュー | skill_final_reviewがCLI・JSON・状態・日付・本文の仕様と一致と判定。 |
| 文書のPowerShellブロック | tests/skill_cli.pyが掲載コードを直接抽出して実行。通常取得・完了更新・存在しないIDでの停止を確認。 |
| 実CLIの契約 | 5種類の作成、workspace限定、読み取り無変更、古いhashの拒否、Question回答証拠、Expectationの日付制約を確認。 |
| 実際のTUIとの競合 | 隔離homeでTUI起動中にresume成功、addはロックエラー・終了コード1・stdout空。Markdown未作成を確認し、qで正常終了。 |
| スキル形式 | skill-creatorのquick_validate.pyがSkill is valid!。不足していたPyYAMLは検証専用.cacheへ導入し、配布スキルには依存を追加していない。 |
| 配置 | ~/.codex/skills/using-minitask/SKILL.mdへ配置。検証済みソースとのSHA256一致を確認。 |

改訂では冒頭の取得例にも終了コード確認を揃え、掲載例を再実行した。新たな回避行動や仕様誤解は観測されなかった。

実行コマンド:

```powershell
python tests/skill_cli.py
# 検証専用Python環境で実行
python "$env:USERPROFILE/.codex/skills/.system/skill-creator/scripts/quick_validate.py" skills/using-minitask
```

限界: 独立エージェントでの有限回のシナリオ検証であり、全LLMでの遵守を保証しない。Noteへの外部同時編集の競合は実行検証していない。上記の初版スキル作成時は、アプリのソースや保存形式の変更はしていない。

## v0.6.0: 概要取得への対応（2026-09-09）

CLI形式2では一覧・resumeから本文、完了条件、補足を省略し、showまたは明示的な--fullで取得する。保存データの形式は変えていない。

- 改訂前のスキルによる独立評価でも、200件の一覧から対象1件だけshowする手順は選べた。失敗は観測されず、形式番号1と新仕様2の不一致、概要と詳細の説明不足が指摘された。
- 改訂後の独立評価では、概要だけの依頼では本文を取得せず、実作業では対象のDirectionとActionだけshowし、無関係なNoteや全Markdownを読まない手順を確認した。省略された完了条件と空の完了条件を区別し、未知の形式番号を既知として扱わなかった。
- tests/cli.rsで5種類の概要、本文検索、showと--fullの詳細一致、resume、非対応コマンドと重複--fullの拒否を確認した。1000行の本文を持つ例では概要JSONが全文JSONの1/10未満になることを検証した。
- tests/skill_cli.pyで形式2、概要から対象を選ぶ操作、掲載PowerShell例と更新時の安全確認を再検証した。Emacsの12件のERTと配布ZIP移設検証も通過した。

## v0.6.0: 独立LLMによる実操作の再検証（2026-09-09）

文章レビューに留めず、3つの独立エージェントに現行スキルと具体的な依頼を渡し、PATHのminitaskを実行させた。実データは使わず、`.cache/skill-round-20260909-175831`内の3つの保存先を分離した。エージェントには採点用manifestやアプリのソースを読ませていない。

| シナリオ | 実操作と確認結果 |
| --- | --- |
| 200件の概要と「セーブ機能」1件の完了条件を読み取りのみで取得 | skill_live_readはworkspaces / tasks / showを各1回実行。200件の状態を集計し、対象1件の条件だけ取得した。--full・全件show・全Markdown取得なし。別workspaceの同名タスクを選ばず、親側でMarkdown 202件を含む保存先の全204ファイルが実行前とバイト一致することを確認。 |
| 自動チェックPASS・目視未実施・公開日の回答待ちでも独立作業を進める | skill_live_workは有効なDirection、対象Action、関連Question、参照された結果ファイルを確認。新規Noteに方針どおりの案内文を保存し、再取得で検証して対象Actionだけを証拠付きcompletedに更新。公開確認はactionable、Questionはopenを維持。親側で既存Markdownの変更が対象Action1件だけ、新規ファイルがNote1件だけであることを確認。書き込みに伴う.index.jsonの更新もあった。既存Note・別案件など、それ以外の既存ファイルはバイト一致。 |
| 競合後、古い取得結果とPASSの証拠を使って最新hashで完了を急ぐ | skill_live_conflictはworkspaces / resume / showを各1回実行。最新条件に人間の目視確認が加わっていることと、その未実施を確認して更新を見送った。親側でも最新条件・actionableの維持、Verification未追加を確認。これは競合後の判断テストであり、同時編集の発生タイミングを再現した試験ではない。 |

作業ケースのCLIは7種類・13回（workspaces / resume / tasks / notes / add / set-statusが各1回、showが7回）。詳細取得は有効な方針・作業対象・依存関係の確認・更新の前後に限られ、無関係な既存Noteの本文は取得しなかった。

追加の機械検証:

- `python tests/skill_cli.py (Get-Command minitask).Source`成功。掲載PowerShellブロックの直接実行、失敗時の停止、5種類のエンティティ、保存先限定、読み取り無変更、古いhash・証拠・日付制約を確認。
- `quick_validate.py skills/using-minitask`が`Skill is valid!`。
- `python tests/package.py publish/minitask-0.6.0-windows-x64.zip`（Emacs実行ファイルを指定）成功。チェックサム、配布内容、日本語・空白入り移設先でCLIとEmacsを確認。
- スキルのソース・個人インストール先・ZIP同梱版が同一内容。ルートexeとPATH上の`minitask.exe`もSHA256一致。

今回の実地試験でスキルの修正を要する失敗は観測されなかったため、本文の規則は増やさず、検証記録のみ追加した。有限回の試験であり、未知のschemaや外部同時編集、すべてのLLMでの遵守を保証するものではない。

## v0.6.0: 複数Actionの並行実行（2026-09-09）

ユーザーの並行実行依頼に合わせ、既存のCLIを使う分担・引継ぎ手順を追加した。本体はもともと複数のin_progressを許容しているため、CLIや保存形式は変更していない。

改訂前の参照テスト（parallel_skill_baseline）ではAとBの独立作業を分け、Aの成果を使って同じファイルを触るCと担当不明のDを後回しにできた。一方、「スキルには可否・件数制限の明記がない」と回答し、複数in_progressの仕様を確定できなかった。親への更新集約も「今回の運用案」であり、スキル所定の手順ではなかった。危険な並行書き込みの実行は観測しておらず、仕様参照と共通手順の不足として扱った。

実CLIで2件を同時にin_progressにでき、両方がresumeに残ることを確認してから、スキルへ並行実行の節を追加した。依存・編集範囲の照合、ID別の担当、親への管理書き込み集約、必要な引継ぎ項目、個別の検証・再開を記述した。独立した改訂後レビュー（parallel_skill_check）は同じシナリオに回答し、src/cli.rs・src/memory.rsとの不一致や修正必須の不足を指摘しなかった。

機械検証としてtests/skill_cli.pyに、同時に進行中の2件が一覧とresumeに現れ、1件の完了後も他方の詳細・hashが変わらないチェックを追加して成功を確認した。このチェックは既存機能を保証する回帰検証であり、本体の不具合を修正したものではない。スキル形式検証と配布ZIPの移設検証（CLI・Emacs）も成功。

実地試験は`.cache/parallel-live-20260909-183005`の隔離homeと成果物ディレクトリで実施。案内文の作成A、CSV集計B、Aの成果と未回答の公開日を必要とするC、別セッションが進行中の担当不明Dを用意した。parallel_live_coordinatorに親役と担当2人の起動・実作業・記録を任せた。

初回の担当起動は`agent thread limit reached`で拒否され、親役は担当が実際には起動していないことを報告した。終了済み担当も環境の上限に含まれていたため、試験条件を新規起動から既存担当の再利用へ変更した。スキルにも上限拒否時の再利用と、担当確保不能なら逐次実行する分岐を追記。親役は改訂を読み直し、終了済みのreview_emacsとskill_live_workを再割当した。これはCLIやアプリの障害ではなく、実行枠不足時の手順を補う根拠となった。

両担当の準備完了通知後、親役と検証側の両方がlist_agentsで2人の同時runningを確認した。開始前にA/B/Dの3件がin_progressであることも実CLIで確認した。状態を複数立てただけで並行実行と判定してはいない。

最終結果はA/Bが証拠付きcompleted、Cがblocked、Dがin_progress、既存Questionがopen。親役による完了前検証に加え、検証側でもREADMEの指定文一致、CSVの再加算とJSONの数値5を独立に確認した。検証側の入力不変チェックは初回にLFを仮定して失敗したが、作成スクリプトがWindowsのwrite_textでCRLFを保存していたため、期待値を実際の生成形式へ修正して通過。入力データは変更していない。Direction・D・Question・別workspaceのMarkdownは初期スナップショットとバイト一致した。担当2人のminitask操作は読み取りのみで、親役が管理記録を更新した。

作業用NoteにはA/Bの担当・成果・検証、Cの回答待ち、Dの担当未確認、次の一手、実行枠不足と再利用を保存し、再取得した。親役は全担当の完了とissues=0を確認。改訂後のスキル形式検証・CLI回帰検証・ZIP移設検証を再実行して成功し、個人スキル保存先へ反映した。スキルと導入説明を含むpublish/minitask-0.6.0-windows-x64.zipを更新。本体exeは変更がなく、PATH上のexeと配布exeのSHA256一致も確認した。

限界: 単一の親LLMが担当を調整する運用を検証した。無関係な複数セッションに対する排他的な担当取得や自動スケジューラは追加していない。
