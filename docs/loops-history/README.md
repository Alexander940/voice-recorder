# Loops history

Un inner loop por historia de usuario (diseñado con la skill `agentic-loops`). Cada carpeta contiene `design.md` (contrato y estado) y `run-<fecha>.jsonl` (log de cada ejecución).

**Kill switch global:** crear `docs/loops-history/DISABLED`.

| Orden | Loop | Depende de |
|---|---|---|
| 1 | [hu-01-scaffold-ci](hu-01-scaffold-ci/design.md) | — |
| 2 | [hu-02-tray](hu-02-tray/design.md) | #1 |
| 2 | [hu-06-paste-output](hu-06-paste-output/design.md) | #1 |
| 3 | [hu-03-hotkey](hu-03-hotkey/design.md) | #1, #2 |
| 4 | [hu-04-audio-capture](hu-04-audio-capture/design.md) | #1, #3 |
| 5 | [hu-05-groq-stt](hu-05-groq-stt/design.md) | #4 |
| 6 | [hu-07-keys-onboarding](hu-07-keys-onboarding/design.md) | #1, #5 |
| 6 | [hu-11-history](hu-11-history/design.md) | #5 |
| 7 | [hu-08-claude-cleanup](hu-08-claude-cleanup/design.md) | #5, #7 |
| 8 | [hu-09-modes](hu-09-modes/design.md) | #8 |
| 8 | [hu-10-vocabulary](hu-10-vocabulary/design.md) | #5, #8 |
| 9 | [hu-12-settings](hu-12-settings/design.md) | #3, #4, #7, #9 |
| 9 | [hu-13-errors-feedback](hu-13-errors-feedback/design.md) | #5, #8, #11 |
| 10 | [hu-14-installers](hu-14-installers/design.md) | MVP |
| 11 | [hu-15-auto-update](hu-15-auto-update/design.md) | #14 |
