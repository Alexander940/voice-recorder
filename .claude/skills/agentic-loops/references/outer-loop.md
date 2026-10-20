# Outer Loops — Design Reference

An outer loop is a **monitoring, strategy, and work-generation cycle**: "decide what work needs doing." It runs on a schedule over days/weeks, never terminates as a program, and its output is not a finished artifact but **triggers, specs, and hypotheses that feed inner loops** — normally with a human approval gate in between. Its strategic value is **grounding**: anchoring the system to external reality (the market, competitors, academic literature, user feedback) instead of letting inner loops optimize in a vacuum.

## Component-by-component guidance

### 1. Trigger
Almost always a **schedule** (daily, weekly). Choose the cadence from the source's natural rhythm: competitor changelogs → daily; academic literature → weekly; intraday market structure → not an outer loop problem, that is streaming infrastructure. Use the platform's native scheduler (cron, systemd timer, scheduled GitHub Action) — do not build a custom scheduler.

### 2. Objective
**Open-ended by design**: "watch X and propose work when something relevant happens." What must still be pinned down precisely:
- The **sources** (exact URLs, feeds, APIs, inboxes).
- The **relevance definition**: what counts as signal. Push the user to make this concrete ("a paper is relevant if it proposes a factor-combination or regime-filter method applicable to our model"), because "anything interesting" produces noise.
- The **output contract**: what a proposal looks like (see handoff spec below).

### 3. Oracle
Outer loops rarely have a hard oracle — that is precisely why they have a human approval gate instead. What you CAN measure and should log:
- **Precision proxy**: fraction of proposals the human approves. If it falls below ~30%, the relevance filter needs tightening.
- **Downstream value**: which approved proposals led to inner-loop successes. This is the real grade, available only over weeks.
A new outer loop should start noisy-but-recallish and tighten based on the approval rate — say this to the user so expectations are set.

### 4. Exit criteria
The loop never ends; **each run does**. Per-run bounds: timeout, max items processed per run, max proposals emitted per run (e.g., top 3 — forced ranking beats exhaustive dumping). Plus a **global circuit breaker**: if K consecutive runs fail (source down, auth expired), disable the schedule and alert the human rather than failing daily forever.

### 5. Memory
The critical component for outer loops. Without it the loop re-proposes the same paper three weeks in a row. Persist:
- **Seen set**: hashes/IDs of every item already examined.
- **Proposal ledger**: every proposal emitted, its status (pending / approved / rejected / superseded), and the human's verdict.
- **Rejection reasons** when available — feed them into the relevance filter so the loop learns the human's taste.
Storage: a small DB table or a committed JSONL file; whatever the repo already uses.

### 6. Human-in-the-loop
The defining feature. Outer-loop decisions are strategic, so the default is **propose → human approves → dispatch inner loop**. Design the approval UX deliberately:
- One message per proposal with: what was found, why it matters, the concrete spec, estimated effort/cost, and an approve/reject affordance (emoji reaction, reply keyword, CLI command, PR review — whatever channel the user already lives in).
- Batch low-priority findings into a digest; interrupt only for high-priority ones.
- **Approval timeout**: a proposal with no response in N days expires (status: stale), it does not auto-execute.

### 7. Executor
Outer loops run unattended, so the executor is a scheduled script (cron, systemd timer, scheduled GitHub Action) or a scheduled cloud agent — never the agent in an open chat session, because the loop must fire when nobody is around. If a run needs judgment (relevance scoring, summarizing), the scheduled job can invoke an agent for that step; the schedule itself still belongs to real infrastructure.

## The handoff spec (outer → inner contract)

The single most important artifact. Every proposal the outer loop emits must be a spec an inner loop can execute without further interpretation:

```markdown
# Proposal: <short title>
**Source:** <url/reference + date found>
**Why it matters:** <2-3 sentences tying it to the system's goals>
**Inner-loop task:** <closed, verifiable objective>
**Oracle for that task:** <how success will be checked>
**Budget:** <max iterations / time / cost>
**Status:** pending approval
```

If you cannot fill in "Inner-loop task" with a verifiable objective, the finding is FYI-only — send it in the digest, do not emit it as a proposal.

## Design doc template

```markdown
# Outer Loop: <name>
> **Status:** DRAFT — pending user approval   <!-- on approval: APPROVED <YYYY-MM-DD> -->
**Classification:** outer loop
**Trigger:** <schedule + scheduler used>
**Executor:** <scheduled script + scheduler | scheduled cloud agent>
**Sources:** <exact feeds/URLs/APIs>
**Relevance definition:** <what counts as signal, concretely>
**Output contract:** handoff specs (max <N> per run) + digest for the rest
**Oracle/metrics:** approval rate, downstream inner-loop outcomes (logged in proposal ledger)
**Per-run bounds:** <timeout, max items, max proposals>; circuit breaker after <K> failed runs
**Memory:** seen set + proposal ledger in <storage>
**Human-in-the-loop:** <channel, approval mechanism, expiry after N days>
**Kill switch:** <how to stop it>
```

## Scaffold pattern (language-agnostic)

```
check_kill_switch_and_circuit_breaker()
items     = collect_from_sources()               # bounded: max items per run
new_items = items - seen_set                      # dedup against memory
scored    = relevance_filter(new_items)           # uses relevance definition + past rejection reasons
proposals = top_k(scored, K)                      # forced ranking, never exhaustive
for p in proposals:
    spec = build_handoff_spec(p)                  # must contain a verifiable inner-loop task
    record_in_ledger(spec, status=pending)
    notify_human(spec)                            # approval affordance included
send_digest(scored - proposals)                   # low-priority findings, batched
update_seen_set(items)
log_run(stats)                                    # items seen, new, proposed, errors
# separately, on approval event:
on_approval(spec): dispatch_inner_loop(spec); update_ledger(approved)
on_expiry(spec, days=N): update_ledger(stale)
```

Implementation notes:
- `collect_from_sources()` failures count toward the circuit breaker; one flaky source must not kill the run (isolate per source).
- Notifications must be idempotent: re-running the loop must not re-notify pending proposals.
- The approval handler is event-driven and separate from the scheduled run.

## Worked example

**Request:** "Watch the literature weekly and feed improvements to my factor model."
- Trigger: weekly cron, Mondays 06:00.
- Sources: arXiv q-fin RSS, SSRN category feed.
- Relevance: proposes a factor-combination, ranking, or regime-filter method testable against the current model's backtest benchmark.
- Output: max 3 handoff specs/run; each spec's inner-loop task = "implement variant, backtest vs. current benchmark on held-out period"; oracle = beats benchmark by declared threshold.
- Memory: seen set of paper IDs + proposal ledger with approve/reject and reasons.
- HITL: one message per spec with ✅/❌ reaction; expires in 7 days; digest for the rest.
- Bounds: 200 papers/run max, 15 min timeout, circuit breaker after 3 failed runs.
