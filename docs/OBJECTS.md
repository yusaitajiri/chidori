# Objects

What a user acts on in Chidori, how those things nest, and how the UI follows from them: object-
oriented UI, where you pick the thing first and then what to do with it. Section 1 lists every
action the app has now (v0.5.0), by where it sits. Section 2 sorts the same actions by the object
they act on. Section 3 is the hierarchy, section 4 the UI built around it, and section 5 the
changes that UI asks for.

## 1. Every action, by where it sits now

**Title bar** (every view)
- 録音 · 履歴 · 設定 (compact: icons for 履歴 and 設定); back (a past session → 履歴, a section → 設定)
- The 録音中 tag: back to the recording from another place; in compact 録音, switch the source
- Always on top; compact / expanded; minimise; close (to the tray while busy, else quit)

**録音, before a recording**
- Source: システム全体, マイクだけ or an app; 更新 the app list
- 会話として録音 on or off; マイクを選ぶ
- 開始
- ファイルから文字起こし; drop an audio or video file on the window
- Compact: open the source panel, close it

**録音, recording or paused**
- Header: rename; project (assign, 新しいプロジェクト…); source dropdown (switch source, 会話として録音)
- Deck: pause / resume; 停止; スクショ; スクショで撮るもの ▾ (the app's window, the cursor's screen,
  all screens, or ウィンドウを選ぶ for this recording); 話している行に重要の印; 区切りを入れる
- Lag notice: 停止後にまとめる; then 今すぐ再開
- Transcript: ★ on any line; 最新へ
- Finishing: 処理を中止

**A finished session** (録音 after 停止, or 履歴 → a session)
- Header: rename; project; 詳細; the saved folder (opens it)
- Deck: 再生 / pause; click the line to jump there; コピー; 書き出し (a dialog: Markdown with a
  ZIP switch, PDF, Typst; compact: a menu); ⋯ (続きを録音, Agent用にコピー, Markdownでコピー,
  フォルダを開く); 新しい録音 (only in 録音)
- A line: ここから再生, ★, 行を編集, 行を削除
- A screenshot: 画像を開く, キャプションを追加, スクショを削除

**履歴**
- Search; filter by project
- Open a session; its row's ⋯: 復元, 続きを録音, 名前を変更, プロジェクト ▸, 書き出し…,
  フォルダを開く, 削除
- 選択: pick rows, all or none, the short and empty ones; プロジェクト ▸; 削除

**設定**
- 一般: language, layout, update check
- 音声: mic, echo guard, keep the sound; advanced: VAD threshold, hangover
- 文字起こし: language, GPU, accuracy first, partials, word list; advanced: hallucination filter,
  audio context
- モデル: download or resume, cancel, 使う, 速度テスト, delete, ほかのモデル, ファイルから追加,
  GPUを再試行
- スクショ: what it takes, leave Chidori out, shutter sound
- ショートカット: start/stop, screenshot, mark, cut
- 出力: save folder, title template, times, labels, merge paragraphs, screenshot markers,
  copy on stop
- プロジェクト: rename, recolour, delete, 新しいプロジェクト
- ログを開く

**Outside the window**
- Hotkeys: start/stop, screenshot, mark, cut
- Tray: 開始/停止, スクショ, ウィンドウを表示, 終了; a click shows or hides the window
- At launch: 復元 or 後で; quitting while recording: 保存して終了 or cancel
- Banners: dismiss, and their one action (mic privacy, switch to the whole system, 停止後にまとめる,
  open the models, open the shortcuts, install the update)
- Setup wizard: next, back, skip; pick a model; download, retry; mic test; shortcuts; finish

## 2. The same actions, by object

Eight objects carry almost every action. The rest are window chrome and preferences.

**Session** (one recording, live or saved). The object the app is about.

| Action | Where now | States |
| --- | --- | --- |
| Create from a source (開始) | start screen, hotkey, tray | ready |
| Create from a file | start screen link, drop | ready |
| Pause, resume, 停止 | deck, hotkey, tray | recording, paused |
| Switch its source | header dropdown, compact 録音中 menu, banner | recording, paused |
| Take a screenshot; choose what it takes | deck, hotkey, tray | recording |
| Mark the line being said | deck, hotkey | recording |
| Cut (start a new part) | deck, hotkey | recording, paused |
| Defer or resume transcription; 処理を中止 | deck | recording, finishing |
| Rename | header (inline), 履歴 row (dialog) | any |
| Assign to a project | header, 履歴 row, 選択 | any |
| Play, jump to a time | deck | saved |
| コピー (plain, Agent用, Markdown) | deck button, ⋯ | saved |
| 書き出し (Markdown, ZIP, PDF, Typst) | deck dialog or menu, 履歴 row | saved |
| フォルダを開く | ⋯, 詳細, 履歴 row | saved |
| 続きを録音 | ⋯, 履歴 row | saved, nothing recording |
| 復元 | launch dialog, 履歴 row | crashed |
| 削除 | 履歴 row, 選択 | saved |
| 新しい録音 (close it, back to the start screen) | deck | saved, in 録音 |

**Source** (what to listen to): pick system, mic only or an app; refresh the apps; 会話として録音;
pick the mic. Edited on the start screen before a recording and in the header while recording;
the mic is also in 設定 › 音声.

**Line** (a segment): play from it, mark or unmark (live too), edit, delete (saved only).

**Screenshot**: open the image (live too), caption, delete (saved only).

**Marker** (cut, pause, source change, reconnect, continued): no actions. A cut starts a **part**,
which exports head with its time but the app does not show as a thing: it has no name, and
nothing jumps to it.

**Project**: create (from any project menu, 設定), rename, recolour, delete (設定 only), filter
履歴 by it.

**Model**: download or resume, cancel, use, benchmark, delete, import from a file; GPUを再試行
belongs to the engine rather than a model.

**Shortcut**: set or clear each of the four. Each is a key for one session action.

Preferences (language, layout, VAD, export defaults, save folder…) are not objects: they are the
defaults new sessions and exports start from.

## 3. The hierarchy

```
Chidori
├── Sessions                       履歴
│   └── Session                    title · project · started · length · model · folder
│       ├── Sources                what it listens to; more join on a switch
│       ├── Sound                  one Opus file per recording; plays from any line
│       └── Timeline
│           └── Part               from the start or a cut to the next cut
│               ├── Line           speaker · time · text · ★ · edited
│               ├── Screenshot     time · image · caption
│               └── Marker         pause · source change · reconnect · continued
├── Projects
│   └── Project → its Sessions     name · colour
├── Models
│   └── Model                      size · speed tier · installed · in use
└── Settings                       defaults: source, screenshots, transcription, export, shortcuts
```

Three things the tree makes plain:

- **A recording is a session in a state**, not a separate object: ready → recording ⇄ paused →
  finishing → saved, and its actions change with the state. The session that just stopped in 録音
  and the one opened from 履歴 are the same object and should look and act the same.
- **Projects are a second way into the sessions**, not a preference. Each session belongs to at
  most one, and a project's natural view is its sessions.
- **The part sits between the session and its lines.** Exports already treat it as a chapter;
  the UI does not show it yet.

## 4. The UI around the objects

The three places stay, each named for what it holds: 録音 is the current session (or the form that
makes the next one), 履歴 is every session, 設定 is the models and the defaults.

**One session view.** Header (title, project, when, numbers, 詳細), timeline, and the deck as
its action bar. It is the same view in 録音 and in 履歴; only how you leave it differs (新しい録音
in 録音, back in 履歴). The deck by state:

| State | Deck |
| --- | --- |
| ready | the source, 開始, ファイルから文字起こし |
| recording, paused | timer · pause · 停止 · スクショ ▾ · ★ · 区切り |
| finishing | progress · 処理を中止 |
| saved | 再生 · コピー ▾ · 書き出し · ⋯ |

**One session menu.** The saved session's ⋯ and a 履歴 row's ⋯ come from one list, built in one
place. The open session leaves out what its deck and header already show; a row, which has
neither, shows it all:

```
復元                     (only a crashed session)
続きを録音
─
コピー ▸                 テキスト · Agent用 · Markdown   (履歴 rows only; the deck has コピー ▾)
書き出し…                 (履歴 rows only; the deck has 書き出し)
─
名前を変更                (履歴 rows only; the header renames inline)
プロジェクト ▸            (履歴 rows only; the header has the swatch)
フォルダを開く
─
削除
```

選択 offers what makes sense for many sessions at once: プロジェクト ▸ and 削除.

**Lines and screenshots.** Their actions stay on hover, under one rule: while a session records,
its timeline only grows, so you can play, star and open but not edit, caption or delete.

**The source is part of making a session.** Its home is the start screen and, while recording,
the header's dropdown. 設定 › 音声's mic and 設定 › スクショ's target stay as the defaults these
edit; the deck's menus change the same settings, as now.

**Projects live in 履歴.** Filtering by a project turns the list into that project's page: the
project's square and name over its sessions, with its own ⋯ (名前と色を変更, 削除). 新しい
プロジェクト… stays at the end of every project menu, the filter's included. 設定 › プロジェクト
is gone.

**Models stay in 設定**: a collection of one kind of thing, used rarely, with its own page.

## 5. Changes this asks for

Done:

1. **One session menu**, used by the deck's ⋯ and 履歴's rows (`lib/sessionMenu.ts`). An open
   session has 削除, and 履歴 rows have コピー.
2. **コピー ▾**, a split button like the camera's: the button copies plain text, the chevron picks
   Agent用 or Markdown. The deck's ⋯ holds only 続きを録音, フォルダを開く and 削除.
3. **Projects live in 履歴**: a filtered list is the project's page, with its menu; 設定 ›
   プロジェクト is gone.

Still to do:

4. **Name the part**: a cut's marker takes a name, which exports use as the part's heading, and
   a long session lists its parts to jump to.

Left as they are, on purpose:

- **ファイルから文字起こし stays a link** under 開始 rather than a source in the list. A file is a
  source (section 1 of the spec says so), but as a list entry it would cost a pick and then 開始,
  where the link starts at once.
- **The deck keeps 停止 and スクショ one tap away** while recording; nothing moves into a menu.
- **Hotkeys and the tray** stay shortcuts to the current session's actions, set in 設定.
