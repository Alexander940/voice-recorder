---
name: agentic-loops
description: >-
  Design and implement agentic loops — recurring autonomous workflows that execute
  tasks (inner loops) or monitor the world and generate work (outer loops). Use this
  skill whenever the user wants to create, design, scaffold, or refactor ANY recurring
  automated process, including a task that runs on a schedule, a watcher/monitor, an
  autonomous agent that iterates toward a goal, a self-correcting pipeline, a bot that
  proposes work, or a cron job with intelligence. Trigger even if the user never says
  "loop" — phrases like "automate this process", "something that checks X every day",
  "an agent that monitors Y and tells me", "keep retrying until tests pass", or their
  Spanish equivalents ("automatiza esto", "que revise X cada dia", "un agente que
  monitoree") all qualify. Also use it when reviewing or debugging an existing loop
  that drifts, hallucinates progress, or spams notifications.
---

# Agentic Loops

A skill for designing and implementing agentic loops: recurring autonomous workflows built around an AI agent or automated process. There are two kinds, and the first job of this skill is always to classify which one the user needs.

| | **Inner loop** | **Outer loop** |
|---|---|---|
| Purpose | Execute a task well | Decide what work needs doing |
| Objective | Closed and verifiable | Open-ended ("watch X") |
| Horizon | Minutes to hours | Days to weeks, runs forever |
| Trigger | Event or direct order | Schedule (cron) |
| Executor | Script on event, or the agent in-chat | Scheduled script or scheduled cloud agent |
| Output | A finished artifact (PR, file, report) | Triggers/specs/hypotheses that feed inner loops |
| Validation | Automatic oracle (tests, validators) | Human approval gate |
| Analogy | The engineer who executes | The PM who prioritizes |

If the user's request mixes both ("monitor arXiv and run experiments on what it finds"), that is an outer loop feeding an inner loop — design them as two separate loops connected by a handoff artifact (a spec), never as one blob. **Design the inner loop first**: the outer loop's output contract (the handoff spec) must contain a task the inner loop can verify, so the inner loop's objective and oracle define what a valid spec looks like.

## Mandatory workflow

1. **Classify** the loop (inner, outer, or a pair). State the classification to the user and why.
2. **Read the matching reference** before designing anything:
   - `references/inner-loop.md` — for execution loops
   - `references/outer-loop.md` — for monitoring/work-generation loops
   - Read both when designing a connected pair.
3. **Run the seven-component gap check** (mandatory protocol below). Do not write code until all seven have concrete answers.
4. **Produce the design doc** using the template in the reference file and hold the approval gate:
   - Write the doc to `docs/loops-history/<loop-purpose>/design.md` with a status header of `DRAFT — pending user approval`, and point the user at the file. Never present the design only as chat text or inside question panels — long docs get truncated there and the user cannot review the complete contract. This applies whether you derived the components yourself or the user supplied them all in the prompt. If `docs/loops-history/` does not exist yet, ask the user before creating it.
   - The gap-check table from step 3 is not the design doc. The file, filled in from the template, is.
   - Only an explicit yes to the doc as written counts as approval. A correction, a new constraint, or any answer that changes one of the seven components is design input, not approval — edit the draft in place and re-present it. What gets executed must be a version the user has seen and affirmed.
   - Ask for approval on its own; never bundle it with execution logistics ("approve the design and run now?"). Bundling is exactly how a correction gets mistaken for a green light.
   - On approval, flip the header to `APPROVED <date>`. Execution may start only once the file says so.
   - Until approval, every action on the system the loop will operate on must be read-only investigation. No state mutations — not even "preparations" like backup tables or staging directories. The draft design doc and its folder are the sole exception: they are documentation, and writing them is how the design is presented.
5. **Build the loop** according to the approved executor: scaffold code adapted to the project's existing stack (see "Stack rules"), or — when the executor is the agent in-chat — execute the iterations yourself, following the reference's skeleton and logging every attempt and oracle verdict to a durable artifact.
6. **Verify the safety floor** (see checklist) before declaring done.

## The seven components

Every loop — inner or outer — must answer all seven questions before a single line of code is written. A loop missing any of these will degenerate: it gets stuck, hallucinates progress, or floods the user with noise.

1. **Trigger — what fires it?** A schedule, an event (new data, failed test, threshold crossed), or a human order/approval. The trigger defines the loop's rhythm.
2. **Objective — what must it produce?** Closed and verifiable for inner loops; open-ended monitoring mandates for outer loops. Never give an inner loop a fuzzy objective ("improve the strategy") — that guarantees drift.
3. **Oracle — how does it know it did well?** A source of truth external to the model: tests, validators, benchmark comparisons, real-world metrics. The most important and most neglected component. An LLM grading its own output is NOT an oracle.
4. **Exit criteria — when does a run end?** Both the success condition AND the failure condition: max iterations, timeout, token/cost budget. Outer loops never end as programs, but every individual run must.
5. **Memory — what state persists between runs?** Git history, a database table, a log of already-tested hypotheses, a set of already-seen items. Without memory, loops repeat work and re-propose the same ideas. For agent-in-chat loops the conversation is working memory, not storage: anything the next iteration or a resuming session needs must live in a durable artifact (see the inner-loop reference).
6. **Human-in-the-loop — what requires approval?** Which decisions pass through the human and over which channel. Inner loops: usually review of the final artifact. Outer loops: approval BEFORE dispatching work.
7. **Executor — who runs the loop, and where?** A script on a scheduler/server, the agent itself iterating in the chat session, or a scheduled cloud agent. Never default silently: the executor determines what the kill switch is, where memory lives, and how failures surface. Finite one-shot inner loops are often best executed by the agent in-chat; anything unattended needs a script or a scheduled agent.

## Gap-check protocol (run before any design or code)

The user's prompt will almost never state all seven components. Verify them explicitly:

1. **Extract** — go through the seven components one by one and classify each as:
   - `provided`: stated in the prompt (quote the user's wording).
   - `inferable`: derivable from the repo or context (existing test suite → oracle candidate; existing cron jobs → scheduler; existing DB → memory store; a finite one-shot task the user wants to watch → agent-in-chat executor candidate). Mark these as assumptions.
   - `missing`: neither stated nor inferable.
2. **Present the table** to the user: the seven components with their status and your proposed value for each `inferable` and `missing` one. Always propose a sensible default for missing components — never ask an open question when you can offer a concrete option to accept or correct (e.g., missing exit criteria → propose "max 4 iterations, 10-minute timeout, escalate with logs").
3. **Ask in one batch.** Collect every confirmation/question into a single message, not an interrogation drip. If only `inferable` items remain (nothing truly missing), state the assumptions and proceed without blocking — the user corrects the design doc if needed.
4. **Hard gate:** if **objective** (for an inner loop) or **oracle** cannot be made concrete even after asking, stop. Do not scaffold a loop with a fuzzy objective or no oracle; explain what instrumentation or definition is needed first.

## Stack rules (this skill is stack-agnostic)

- **Inspect the repo first.** Use the project's existing language, scheduler, database, and notification channel. A Python repo gets a Python loop; a TypeScript repo gets TypeScript; if they already use GitHub Actions or systemd timers or Airflow, schedule there.
- **Greenfield default:** a single script + the platform's native scheduler (cron/systemd on Linux, scheduled GitHub Action if the project lives on GitHub) + a plain file or SQLite for memory. Do not introduce queues, frameworks, or new infrastructure unless the user asks or scale demands it.
- **The design doc is stack-free; the scaffold is stack-specific.** Components and contracts first, technology second.

## Safety floor (verify before declaring done)

Every scaffold must ship with all of these, no exceptions:

- [ ] A **kill switch**: one obvious way to stop the loop (disable the schedule, a `DISABLED` flag file, an env var checked at startup). For agent-in-chat loops the kill switch is interrupting the agent — which does NOT cancel detached operations already in flight, so keep each side effect atomic and verifiable after an interrupt.
- [ ] **Bounded runs**: max iterations and a timeout enforced in code, not just documented.
- [ ] **Structured logging** of every run: trigger time, actions taken, oracle verdict, exit reason.
- [ ] **Failure escalation**: when a run exhausts its budget or errors, it notifies the human — it never retries silently forever.
- [ ] **Idempotence or dedup**: re-running with the same inputs must not duplicate side effects (double orders, duplicate PRs, repeated notifications).
- [ ] **No irreversible actions without explicit human approval** (spending money, sending external communications, trading, deleting data). This holds even if the user's first prompt implies full autonomy — confirm it.

## Anti-patterns to refuse or push back on

- An inner loop whose objective cannot be verified by its oracle ("make the code better").
- Self-grading: the same model that produced the output deciding the output is good.
- An outer loop that auto-executes its proposals with no approval gate on consequential actions.
- A loop with no memory that re-discovers the same finding every run.
- Unbounded retry ("keep trying until it works") with no iteration cap or budget.
- One mega-loop that monitors AND executes AND evaluates — decompose it.
