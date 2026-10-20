# Inner Loops — Design Reference

An inner loop is an **execution cycle with a closed, verifiable objective**: "do this task well." It runs for minutes to hours, is validated by an automatic oracle, and has a clear end condition. Examples: fix failing tests and open a PR; generate a portfolio weights file that satisfies constraints; produce a report with exactly N required sections.

## Component-by-component guidance

### 1. Trigger
Inner loops fire on **events or direct orders**: a spec arrives from an outer loop, a test fails in CI, a threshold is crossed, the user runs a command. If the user wants a schedule-triggered inner loop ("every night, fix lint errors"), that is fine — but check whether there is an implicit outer loop hiding there (deciding *which* errors matter).

### 2. Objective
Must be **closed and verifiable**. Test: could a script with no judgment decide whether the objective was met? 
- Good: "PR open with CI green", "weights file where every constraint validator passes", "all 12 sections present and non-empty".
- Bad: "improve performance", "clean up the code", "make the strategy better". If the user gives a fuzzy objective, negotiate it into a measurable one before proceeding — propose the measurable version and confirm.

### 3. Oracle
The oracle is **external to the model** and ideally external to the loop's own code. Patterns, strongest first:
1. **Real-world feedback**: actual metrics (response rates, fill prices, prod errors). Slow but unfakeable.
2. **Executable checks**: test suites, compilers, constraint validators, schema checks.
3. **Benchmark comparison**: candidate output vs. a fixed baseline on held-out data (e.g., backtest vs. benchmark). Beware overfitting to the benchmark across many iterations.
4. **Independent LLM judge with a rubric**: weakest option; only acceptable for subjective qualities, never as the sole oracle, and never the same context that generated the output.

If no oracle can be defined, stop: the task is not ready to be an inner loop. Tell the user what instrumentation they would need first.

### 4. Exit criteria
Define all three:
- **Success**: oracle passes.
- **Failure budget**: max N iterations (default 3–5) AND a wall-clock timeout AND optionally a cost budget. Enforced in code.
- **On failure**: stop, persist the partial work and the log of attempts, escalate to the human with a diagnosis. Never loop silently past the budget.

### 5. Memory
Inner-loop memory is mostly **within-run**: what was attempted, what the oracle said each time, so iteration N+1 actually learns from iteration N. Persist it in the natural medium: Git history for code, a run log (JSONL or DB row) for everything else. Cross-run memory (e.g., "this class of fix never works") belongs to the outer loop that supervises it.

**When the executor is the agent in-chat**, the conversation feels like memory but is only working memory: the session can end at any point, and long runs get compacted/summarized, which silently distorts precise values (IDs, counts, oracle verdicts). Treat the conversation as RAM and a durable run log as disk:

- **The design doc is already durable**: it lives in the loop's folder from draft stage (workflow step 4), and approval flips its status header to `APPROVED`. A resuming session recovers the contract — objective, oracle, exit criteria, and whether it was ever approved — without renegotiating it.
- **Write-ahead logging**: record the intent (item, action, timestamp) *before* each side effect, and the outcome (oracle verdict, evidence, backups created) *after* it. An intent with no matching outcome means possibly in-flight or interrupted work — on resume, verify the real state before retrying; never assume the action didn't happen.
- **Per-item status** for multi-item loops: `pending / in-progress / done / failed` plus attempt count. Resuming is then mechanical: skip `done`, verify `in-progress` against reality, continue with `pending`.
- **After a context compaction, re-read the log** instead of trusting recalled values. The log is the source of truth; the summary is not.
- **Keep the log factual and lean** — actions, verdicts, artifact pointers. Reasoning and dead ends can stay in the conversation.

**Where the files live.** In-chat loop memory has a fixed home so a resuming session knows where to look without being told: `docs/loops-history/<loop-purpose>/` at the project root.

```
docs/loops-history/
└── merge-duplicate-tickers/      # kebab-case, named for the problem the loop solves
    ├── design.md                 # status header DRAFT → APPROVED <date>; gains an ## Outcome section on completion
    ├── run-2026-06-11.jsonl      # one write-ahead log per run, date-named (-2, -3 for same-day reruns)
    └── run-2026-07-02.jsonl      # a later re-run of the same loop lands in the same folder
```

- Name the folder for the *problem*, the run files for each *execution*. Re-running the loop weeks later accumulates history in one place instead of scattering new folders.
- The loop's folder is created when the draft design doc is written (workflow step 4). If the base `docs/loops-history/` directory doesn't exist yet, ask the user before creating it — first-time setup of the convention is the user's call.
- Each JSONL line is one event: `{"ts", "run_id", "item", "phase": "intent" | "outcome", "action", "verdict", "evidence", "artifacts"}` — `verdict`, `evidence` and `artifacts` appear only on outcome lines.
- When the loop ends — success or escalation — append an `## Outcome` section to `design.md` with the verdict per item and the exit reason, so the folder reads as a complete record without parsing the JSONL.
- If the project is a git repo, commit the folder: it is the audit trail of an intervention on the system. Never gitignore it.
- If the loop doesn't run inside a project with a clear root, ask the user where to anchor the history folder.

Do not create tables or other objects in the project's database for loop memory — that touches real infrastructure and can break things; it is an option only when the user explicitly puts it on the table.

### 6. Human-in-the-loop
Default: human reviews the **final artifact** (the PR, the report, the weights file) — not every iteration. Exception: any irreversible side effect (deploying, trading, sending email) requires approval *before* execution regardless of oracle verdict.

### 7. Executor
Where the loop physically runs — ask or propose it, never assume:
1. **A script** wired to the trigger (event hook, scheduler) — the default for anything recurring or unattended.
2. **The agent in-chat**: the agent iterates inside the session, running each attempt and oracle check as tool calls. The right fit for finite, one-shot loops (a bounded backlog, a migration over N items) where the user wants to watch and steer. Design implications: memory needs the write-ahead protocol described in section 5 — the conversation alone dies with the session; the kill switch is the user interrupting the agent, which does NOT stop detached operations already launched (a background transaction keeps running through the interrupt), so make each side effect atomic and verifiable after the fact.
3. **A scheduled cloud agent** when the user wants agent judgment on a schedule without keeping a session open.

## Design doc template

Produce this and get approval before scaffolding:

```markdown
# Inner Loop: <name>
> **Status:** DRAFT — pending user approval   <!-- on approval: APPROVED <YYYY-MM-DD> -->
**Classification:** inner loop
**Trigger:** <event/order/schedule + exact mechanism>
**Executor:** <script + where it runs | agent in-chat | scheduled cloud agent>
**Objective (verifiable):** <artifact + pass condition>
**Oracle:** <what checks it, how it is invoked, what "pass" means>
**Exit criteria:** success = <...>; failure = max <N> iterations OR <timeout> OR <budget>; on failure → <escalation>
**Memory:** <where attempt history lives; agent in-chat → docs/loops-history/<loop-purpose>/ (see the Memory section)>
**Human-in-the-loop:** <what is reviewed/approved, over which channel>
**Irreversible actions:** <list, each gated by approval | "none">
**Kill switch:** <how to stop it>
```

## Scaffold pattern (language-agnostic)

Adapt to the repo's stack. The skeleton every inner loop shares:

```
load_config_and_check_kill_switch()
context = load_task_spec()
history = []                      # within-run memory
for attempt in 1..MAX_ITERATIONS:
    check_timeout_and_budget()
    result = execute_step(context, history)      # the agent/work happens here
    verdict = oracle(result)                     # external check, never self-grading
    log_run(attempt, result.summary, verdict)
    if verdict.passed:
        deliver_artifact(result)                 # PR / file / report
        notify_human_for_review()
        exit(success)
    history.append((result.summary, verdict.feedback))
escalate_to_human(history)                       # budget exhausted
exit(failure)
```

Implementation notes:
- `oracle()` must be callable standalone (so the human can run it on any candidate output).
- `log_run()` writes structured records: timestamp, attempt, action summary, oracle verdict, exit reason.
- `deliver_artifact()` must be idempotent (re-delivery does not duplicate PRs/orders/files).
- The kill switch check happens at startup AND at the top of each iteration for long runs.
- When the executor is the agent in-chat, this skeleton is the discipline the agent follows, not code to ship: each iteration is one round of tool calls, `log_run()` writes to a durable artifact, and interrupts land between iterations — detached work launched inside an iteration outlives them.

## Worked example

**Request:** "Every time my constraint config changes, regenerate the portfolio weights file."
- Trigger: file-change event on `constraints.yaml` (or CI hook).
- Objective: `weights.csv` covering all tickers in the universe.
- Oracle: standalone validator script — weights sum to 1, sector caps respected, per-ticker bounds respected, no NaNs.
- Exit: success = validator passes; failure = 4 iterations or 10 minutes → escalate with validator errors attached.
- Memory: run log table (run id, constraint hash, attempts, verdict).
- HITL: human approves before any downstream order execution; weights file itself needs no pre-approval.
