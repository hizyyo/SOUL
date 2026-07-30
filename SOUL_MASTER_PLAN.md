# SOUL

## Master Product, Business and Architecture Plan

**Version:** 1.0  
**Date:** July 29, 2026  
**Status:** fundraising prototype and product specification  
**Core thesis:** models are replaceable; the executable model of a person must belong to that person.

---

## Кратко Для Основателя

Основной документ написан на английском, чтобы его можно было сразу отправлять ангелам, YC-партнерам, advisors и техническим экспертам.

Главные зафиксированные решения:

1. SOUL не является еще одной памятью или вторым мозгом. Это пользовательский runtime контекста, решений и полномочий.
2. Первый wedge: за пять минут собрать полезный локальный `.soul` через интерактивную калибровку и подключить его к уже используемому AI-инструменту.
3. Blind Preference Test измеряет, какой ответ больше подходит пользователю. Он не доказывает предсказание реального решения.
4. Prospective Decision Test заранее фиксирует прогноз и открывает его только после реального выбора пользователя.
5. Agent governance считается настоящим enforcement только тогда, когда credential и выполнение проходят через SOUL Gateway.
6. Агент не должен владеть прямым API key сервиса, иначе он может обойти SOUL.
7. 75-85% операций должны выполняться детерминированно и без токенов.
8. Heavy reasoning должен использоваться менее чем в 2% policy-решений.
9. Типичный context frame ограничен 400-900 токенами, абсолютный cap составляет 3,000 токенов.
10. Локальный policy hot path должен укладываться в p95 до 5 мс, полный local context path в p95 до 75 мс.
11. Fundraising prototype строится за 14 дней, после чего идет отдельное двухнедельное тестирование на внешних пользователях.
12. Free-версия всегда сохраняет локальное владение, экспорт и удаление данных.
13. Plus стоит $12/месяц или $99/год. Operator стоит $29/месяц или $240/год.
14. Нельзя продавать lifetime sync и безлимитный heavy reasoning.
15. Главный стоп-критерий: если полный SOUL не побеждает простой вручную проверенный profile prompt, продукт нужно упрощать или менять wedge.

Ключевая формула:

```text
Calibrate
→ Connect
→ Work in the existing AI client
→ Improve
→ Govern through Gateway
```

---

## 0. Executive Summary

SOUL is a local-first, user-owned runtime that represents how a person remembers, prefers, decides, communicates and delegates authority to AI systems.

SOUL is not another chatbot, memory database, vector store or static user profile. It is a verifiable context layer across AI models and an authority layer for agent execution paths integrated through its Gateway.

```text
Models provide intelligence.
SOUL provides the user.
Policies preserve the user's authority.
Evals measure whether personalization is real.
```

The product has two jobs:

1. **Personalization:** compile the minimum relevant context that lets any model act more like the user.
2. **Governance:** decide what agents may know and do on the user's behalf.

The product must prove both jobs independently:

```text
Blind Soul Test
Proves that answers generated with SOUL beat the same model without SOUL.

Soul Policy Engine
Applies deterministic boundaries and permissions before expensive reasoning.
```

The first magical demo is deliberately narrow:

```text
Create a local .soul through a five-minute guided calibration
→ connect SOUL to an AI client the user already uses
→ type the task in that client, not in SOUL
→ inject a minimal task-specific context through the integration
→ preserve decisions, preferences and boundaries across clients
```

The first investor story is not "we have invented digital consciousness." It is:

> Every AI product currently builds a different, hidden and often incorrect model of its user. SOUL gives the user one inspectable and measurable model that works across providers, plus an enforceable gateway for agent actions.

The first 14-day fundraising prototype must optimize for one result:

> A stranger creates a useful local SOUL without external history, connects it to an existing AI client and immediately sees the client receive relevant preferences and decisions. A separate blind test measures whether SOUL beats a strong simple profile baseline.

The 90-day beta adds agent governance:

> The same SOUL that accumulates prospective decision evidence becomes the authority layer that allows, blocks, redacts or escalates actions executed through SOUL Gateway.

No product can guarantee funding on launch day. The objective is to create the strongest possible fundraising artifact: a ten-second demo, a new category, measurable technical proof, early user evidence and a believable path from a small desktop app to personal intelligence infrastructure.

---

## 1. The Category

### 1.1 Category name

Primary category:

> **Personal Intelligence Runtime**

Secondary technical description:

> **Verifiable Human Intent Runtime for AI Agents**

Consumer description:

> **An AI model of you that you own.**

### 1.2 One-line pitch

> SOUL learns how you decide, proves it through blind tests, carries that identity across every AI and governs what agents can know and do on your behalf.

### 1.3 What SOUL is not

SOUL is not:

- a second brain;
- a note-taking app;
- a ChatGPT wrapper;
- a generic memory API;
- a universal vector database;
- a personality prompt;
- a digital consciousness claim;
- an autonomous agent that silently controls the computer;
- an enterprise compliance dashboard in the first release;
- a marketplace before the core runtime works.

### 1.4 The new primitive

Obsidian has a vault. Git has a repository. Docker has an image. SOUL needs an equally legible object:

```text
ilya.soul
```

A `.soul` is a portable, versioned and inspectable package containing:

- identity facts;
- contextual preferences;
- goals;
- important relationships;
- past decisions and reasons;
- hard boundaries;
- agent permissions;
- communication styles;
- provenance;
- personal evaluation cases;
- policy versions;
- cryptographic history.

The `.soul` file can be:

- opened;
- inspected;
- corrected;
- encrypted;
- exported;
- imported;
- backed up;
- connected to a model;
- disconnected from a model;
- branched for a context;
- rolled back;
- used by an agent for a defined purpose;
- cryptographically erased and purged according to an explicit deletion policy.

### 1.5 The long-term platform

The initial product transfers and verifies personal context. The platform eventually provides:

```text
soul.context(task)
soul.decide(options)
soul.rank(items)
soul.draft(message)
soul.review(action)
soul.authorize(action)
soul.explain(decision)
soul.learn(correction)
```

The long-term position is not "another AI assistant." It is the principal layer to which integrated assistants and governed agent execution paths answer.

---

## 2. Why Now

### 2.1 The market shift

AI systems have moved through three stages:

```text
2023-2024: models answered questions
2024-2025: assistants retained context and used tools
2025-2026: agents began acting across files, browsers, APIs and workflows
```

As capability increases, fragmented identity becomes more dangerous:

- one model remembers outdated preferences;
- another model invents a preference from one conversation;
- an agent receives more private context than it needs;
- a coding agent and a shopping agent receive the same profile;
- a user cannot verify whether personalization is actually better;
- every vendor owns a separate model of the same person;
- autonomous agents need stable boundaries, not only richer memory.

### 2.2 The competitive reality

The simple pitch "portable memory across AI apps" is already occupied.

As of July 29, 2026:

- Egoist Machines, YC Summer 2026, publicly describes an AI Passport for user-owned memory and preferences across AI applications.
- ChatGPT memory automatically synthesizes context from conversations, files and connected applications.
- Mem0 sells production memory infrastructure to developers.
- multiple local-first memory projects and standards are emerging.

Therefore the defensible category cannot be just:

```text
one memory for every AI
```

SOUL must own the next layer:

```text
verified personalization
+ executable decisions
+ authority and policy enforcement
+ minimal-purpose context disclosure
```

### 2.3 The window

The opportunity remains open because current products generally optimize one of the following:

- memory retrieval;
- personalization inside one vendor;
- portable profile storage;
- enterprise knowledge retrieval;
- agent orchestration;
- security rules without a personal decision model.

SOUL combines four elements that are individually useful but strategically stronger together:

```text
Typed personal state
Provenance and corrections
Blind personalization evaluation
Deterministic agent governance
```

The non-commodity differentiation must be stated precisely:

```text
Preference proof: controlled blind tests against a strong simple profile baseline
Decision proof: prospective predictions scored only after the real outcome exists
Governance proof: a mandatory credential-holding Gateway, not a cooperative policy callback
```

Portable memory, approval inboxes, scoped access and receipts are useful product requirements, but they are no longer sufficient differentiation by themselves.

---

## 3. Product Thesis

### 3.1 Memory is not the product

Memory answers:

> What has the user said or done before?

SOUL answers:

> What information matters for this task, what would the user probably choose, what is forbidden and how confident are we?

### 3.2 The three product modes

#### Mirror

Mirror helps the user inspect and correct the system's model of them.

Functions:

- see inferred preferences;
- inspect evidence;
- identify contradictions;
- compare stated values with repeated decisions;
- expire an old preference;
- mark "this is no longer me";
- run blind Soul Tests;
- compare branches such as `work`, `public`, `private` and `founder`.

#### Delegate

Delegate helps produce decisions and outputs in the user's style.

Functions:

- draft a message;
- rank alternatives;
- summarize likely tradeoffs;
- prepare a negotiation position;
- recommend an action with confidence;
- identify what missing information would change the answer;
- learn from an override.

#### Guardian

Guardian controls other agents.

Functions:

- allow or deny context access;
- redact sensitive data;
- enforce spending limits;
- require confirmation;
- block irreversible actions;
- maintain an action receipt;
- verify the effect after a tool call;
- record an override as a possible future policy.

### 3.3 Product loop

```text
Observe evidence
→ propose typed memory or decision
→ user confirms or corrects
→ compile context or policy
→ model or agent acts
→ user accepts or overrides
→ SOUL records the outcome
→ blind eval verifies improvement
```

Each correction is more valuable than a raw conversation because it reveals the difference between generic model behavior and the user's actual choice.

---

## 4. Initial Customer and Wedge

### 4.1 First user

The first ideal user is an AI power user who:

- uses at least two model providers weekly;
- regularly repeats project context;
- cares about model portability;
- uses coding or research agents;
- understands local-first ownership;
- is willing to test early desktop software.

Examples:

- founder;
- developer;
- researcher;
- writer;
- investor;
- student applying to competitive programs;
- content creator with a strong writing identity.

### 4.2 First pain

The first pain is not "I need a clone." It is:

> My current AI knows my projects and preferences, but every new model starts from zero or builds a different version of me.

### 4.3 First promise

> Use one owned identity inside the AI tools you already use, without moving your work into another chat application.

### 4.4 First use case

```text
Create a local SOUL through guided calibration
→ connect Codex, Claude Code, Cursor or another supported client
→ keep writing tasks in that existing client
→ the client requests a minimal task-specific context frame from SOUL
→ optional imports and confirmed corrections improve SOUL later
→ a separate blind test measures personalization against B1
```

### 4.5 Why this wedge is strong

- It has a clear before and after.
- It works before the user imports any external data.
- It creates value on the first day.
- It naturally demonstrates provider independence.
- It produces a shareable score.
- It creates an initial personal evaluation dataset.
- It leads directly to agent governance.

### 4.6 Why not start enterprise-first

Enterprise governance can produce larger contracts, but it creates early traps:

- long security reviews;
- custom integrations;
- consulting-heavy pilots;
- unclear product boundaries;
- slow feedback;
- pressure to become a generic policy platform.

The recommended sequence is:

```text
Consumer/prosumer proof of personal model
→ developer integration
→ agent governance
→ team and enterprise controls
```

An enterprise design partner can be accepted early, but it must use the same core runtime rather than creating a separate product.

This sequence is a capital-efficient product hypothesis, not proof that consumer distribution naturally converts into enterprise demand. Consumer and enterprise funnels must be measured separately. If governance buyers do not emerge from developer integrations, SOUL should run a direct B2B motion or keep governance as a separate product line rather than forcing a false consumer-to-enterprise story.

---

## 5. The Magical First Experience

### 5.1 Time to value

Target:

```text
Install to useful local SOUL: under 5 minutes
Install to first connected client: under 7 minutes
Install to first SOUL-assisted task in that client: under 8 minutes
Install to first blind comparison: under 12 minutes
Complete a 20-round blind test: 8-15 minutes of active user time
```

Optional archives continue processing in the background after activation. Import, parsing and extraction must never block client connection or SOUL-assisted work.

### 5.2 Onboarding

```text
Step 1: Create local SOUL
Step 2: Complete a five-minute guided calibration
Step 3: Review and activate the compact initial SOUL
Step 4: Connect one existing AI client
Step 5: Type a normal task in that client and inspect what SOUL shared

Optional later: Improve SOUL with a ChatGPT export, writing samples or corrections
```

SOUL has no general-purpose chat window. The desktop application is a local runtime and control center for calibration, integrations, disclosure receipts, candidate review, permissions, export and account state. Users continue working in Codex, Claude Code, Cursor, ChatGPT, Claude or other existing clients.

### 5.3 Required screens

#### Home

```text
SOUL MATCH: 74%
12 decisions verified
3 candidate updates
2 connected AI clients
0 policy violations
```

#### Candidate Inbox

```text
SOUL inferred:
"Prefer fast iteration over architectural purity during MVPs."

Evidence:
3 decisions across 2 projects

[Confirm] [Edit] [Reject]
```

#### Blind Test

```text
Which answer is more like you?

[A] Decline the offer because...
[B] Accept only if...

[A] [B] [Neither]
```

The user must not know which answer used SOUL.

#### Context Preview

```text
Claude Code requests context for:
"Refactor authentication flow"

SOUL will share:
4 architecture decisions
2 coding preferences
1 hard boundary

Sensitive personal context: not shared

[Allow once] [Always for this project] [Deny]
```

#### Decision Explanation

```text
Likely choice: Decline
Confidence: 82%

Based on:
2 similar decisions
1 current goal
1 spending boundary

Would change if:
The offer included distribution or strategic access.
```

---

## 6. SOUL Evaluation System

### 6.1 Purpose

The evaluation system exists to prevent astrology, Barnum effects and self-congratulatory model scoring.

It separates two different claims:

```text
Blind Preference Test
Measures whether the user prefers a SOUL-personalized answer over a controlled baseline.

Prospective Decision Test
Measures whether SOUL predicts a later real choice that was hidden from the system.
```

The first is a product-personalization metric. The second is the only basis for a decision-prediction claim. Neither proves a complete model of a human personality.

SOUL must never claim:

```text
Your clone is 94% accurate
```

unless the number comes from a pre-defined, blind and reproducible evaluation protocol.

### 6.2 Blind Preference Test protocol

For each held-out scenario:

1. The same model generates two answers.
2. Variant A receives the compiled SOUL context.
3. Variant B receives no SOUL context or a defined baseline profile.
4. Model, temperature, tools, token budget and source question are held constant.
5. Answer order is randomized.
6. Style and length are normalized where possible.
7. The user chooses A, B or Neither.
8. The user does not know which variant is SOUL.
9. The system records the choice before revealing the variant.

### 6.3 Baselines

SOUL must beat more than an empty prompt.

Required baselines:

```text
B0: same model with no personalization
B1: same model with a short manually written user profile
B2: same model with generic chat summary
B3: SOUL without decision history
B4: SOUL without preferences
B5: SOUL with deliberately irrelevant memories
```

If full SOUL does not outperform a 10-line profile prompt, the system is too complex for the value it delivers.

MVP requirement:

```text
Required at launch: B1 short manually reviewed profile
Required for the public benchmark: B0-B5 ablation suite
```

The 14-day product does not need to run six variants during every onboarding. It must, however, compare against B1 rather than an artificially empty baseline.

### 6.4 Held-out data

Evaluation questions must not be copied from training conversations.

Sources:

- newly authored dilemmas;
- decisions created after import;
- counterfactual variations of past decisions;
- tradeoffs that combine two known principles;
- adversarial questions designed to trigger generic agreement;
- questions where the correct answer is "insufficient information".

### 6.5 Metrics

Primary metric for the Blind Preference Test:

```text
Soul Win Rate = Soul wins / (Soul wins + baseline wins)
```

Ties are reported separately.

Secondary metrics:

- decision match;
- reasoning match;
- communication style match;
- boundary recall;
- confidence calibration;
- factual contradiction rate;
- unnecessary personalization rate;
- user edit distance;
- time to acceptance;
- input tokens;
- output tokens;
- latency;
- cost.

### 6.6 Statistical reporting

The MVP report must show:

- number of tests;
- wins;
- losses;
- ties;
- exact binomial p-value on non-ties;
- Wilson 95% confidence interval;
- model and version;
- baseline definition;
- test creation method;
- whether questions were held out;
- total tokens and cost.

An example share card:

```text
SOUL BLIND TEST

Tests: 48
SOUL wins: 35
Baseline wins: 9
Neither: 4

Win rate: 79.5%
95% CI: 65.5%-88.8%
Model: same for both variants
```

### 6.7 Minimum credible thresholds

For an individual onboarding demo:

- at least 20 tests;
- report the result as directional personal evidence, not statistical proof;
- ties shown explicitly;
- no claim of statistical certainty if the interval crosses 50%;
- encourage an extended 50-100 round test for users who want stronger evidence.

For a public product claim:

- at least 100 users;
- held-out tests;
- cluster bootstrap or mixed-effects analysis by user;
- lower 95% confidence bound above 50%;
- absolute lift of at least 5 percentage points over the strongest simple baseline;
- no increase in boundary violations.

### 6.8 Prospective Decision Test

To claim that SOUL predicts decisions, use a different protocol:

1. Register a real unresolved decision before the user chooses.
2. Freeze the SOUL state and model version.
3. Generate a prediction, confidence and conditions that would change it.
4. Hide the prediction until the user records the actual choice.
5. Record whether the final choice matched, differed or was invalidated by new information.
6. Prevent the unresolved decision and final outcome from entering extraction before scoring.
7. Evaluate accuracy, calibration and abstention over many future decisions.

Primary metrics:

- exact option match;
- top-k match for multi-option choices;
- Brier score;
- calibration error;
- abstention coverage and accuracy;
- decision changes caused by new information;
- prediction lift over a simple user-profile baseline.

The fundraising MVP can demonstrate the workflow with founder decisions, but public prediction claims require prospective data collected after launch.

### 6.9 Anti-gaming controls

- Randomized A/B position.
- `Neither` is always available.
- Placebo rounds with two baseline answers.
- Negative-control memories.
- Equal token budgets.
- Equal model versions.
- No reveal until the choice is committed.
- No cherry-picked public reports.
- Fixed-horizon tests for published benchmarks.
- Separate product analytics from benchmark datasets.

### 6.10 Viral loop

The Blind Preference Test result is a natural social artifact:

```text
I preferred my SOUL-personalized answer in 37 of 48 blind comparisons.
Could your AI actually tell you apart from everyone else?
```

The share card must never expose private questions or decisions by default.

---

## 7. Core Data Model

### 7.1 Entity types

```ts
type SoulEntityType =
  | "fact"
  | "preference"
  | "boundary"
  | "decision"
  | "policy"
  | "goal"
  | "relationship"
  | "routine"
  | "communication_style";
```

### 7.2 Common entity shape

```ts
interface SoulEntity {
  id: string;
  type: SoulEntityType;
  namespace: string;
  subject: string;
  status:
    | "candidate"
    | "active"
    | "disputed"
    | "superseded"
    | "expired"
    | "rejected"
    | "deleted";

  scope: {
    domains: string[];
    projects: string[];
    people: string[];
    channels: string[];
  };

  confidence: number;
  importance: number;
  sensitivity: "public" | "internal" | "private" | "sensitive" | "restricted";
  stability: "ephemeral" | "situational" | "stable";

  validFrom?: string;
  validUntil?: string;
  evidenceIds: string[];
  supersedes?: string[];
  conflictsWith?: string[];
  createdAt: string;
  updatedAt: string;
}
```

### 7.3 Preference

```ts
interface PreferenceEntity extends SoulEntity {
  type: "preference";
  value: string;
  strength: number;
  exceptions: string[];
  alternatives: string[];
}
```

### 7.4 Boundary

```ts
interface BoundaryEntity extends SoulEntity {
  type: "boundary";
  hardness: "soft" | "hard" | "immutable";
  actionKinds: string[];
  effect: "deny" | "require_confirmation" | "redact";
}
```

### 7.5 Decision

```ts
interface DecisionEntity extends SoulEntity {
  type: "decision";
  question: string;
  options: string[];
  selected: string;
  reasons: string[];
  rejectedReasons: string[];
  conditionsThatWouldChangeDecision: string[];
  outcome?: string;
}
```

### 7.6 Provenance

Every entity must be traceable.

```ts
interface Provenance {
  id: string;
  sourceKind:
    | "explicit_user"
    | "conversation"
    | "document"
    | "calendar"
    | "email"
    | "tool_result"
    | "agent_inference";
  sourceLocator?: string;
  observedAt: string;
  capturedHash?: string;
  excerpt?: string;
  trustLevel: number;
  consent: "explicit" | "implicit_session" | "imported";
  transformations: {
    processor: string;
    version: string;
    promptHash?: string;
    outputHash: string;
  }[];
}
```

### 7.7 Candidate lifecycle

```text
observed
→ extracted
→ candidate
→ active
→ reaffirmed
→ superseded / expired / deleted
```

Alternative paths:

```text
candidate → rejected
candidate → needs confirmation
active → disputed
disputed → active / superseded
```

### 7.8 Activation rules

| Input | Default result |
|---|---|
| User explicitly creates a memory | Active |
| User says "remember this" | Active with undo |
| Explicit low-risk preference | Active with preview |
| Inferred preference | Candidate |
| Sensitive inference | Confirmation required |
| New hard boundary | Explicit confirmation |
| Weakened hard boundary | Explicit confirmation plus audit |
| Repeated observation | Add evidence, do not silently create truth |
| Contradiction | Disputed, never last-write-wins |

---

## 8. The `.soul` Format

### 8.1 Package structure

```text
ilya.soul
├── manifest.json
├── events/
│   └── events.cborseq
├── snapshots/
│   └── state.cbor
├── blobs/
│   └── sha256/<hash>
├── policies/
│   └── compiled.cbor
├── schemas/
│   └── soul-state.schema.json
├── tests/
│   └── soul-tests.jsonl
├── keys/
│   └── public.json
└── signatures/
    └── package.sig
```

### 8.2 Format choices

- deterministic ZIP64 container;
- UTF-8 text;
- JSON for manifest and schemas;
- canonical CBOR for event streams and snapshots;
- SHA-256 content addressing;
- Ed25519 signatures;
- XChaCha20-Poly1305 encryption;
- independent semantic and package versions;
- no executable native code inside the package.

### 8.3 Example manifest

```json
{
  "format": "soul-package",
  "formatVersion": "0.1.0",
  "schemaVersion": "0.3.0",
  "soulId": "soul_01K...",
  "createdAt": "2026-07-29T12:00:00Z",
  "headEvent": "sha256:...",
  "snapshotEvent": "evt_...",
  "encryption": {
    "mode": "xchacha20-poly1305",
    "keyEnvelope": "keys/envelope.cbor"
  },
  "capabilities": [
    "typed-memory",
    "policies",
    "blind-tests"
  ]
}
```

### 8.4 Import security

Every imported package passes:

1. size and nesting limits;
2. manifest validation;
3. hash verification;
4. signature verification;
5. schema validation;
6. sandbox migration;
7. human-readable preview;
8. explicit confirmation for policies and boundaries.

The package must not execute JavaScript, Python, shell, WASM or remote network calls during import.

Package signatures establish integrity and device provenance, not legal identity. The trust model is:

- each device has a locally generated signing key;
- the first device becomes the initial trusted root for that SOUL;
- additional devices are approved by an already trusted device or a recovery key;
- unsigned packages can be imported only as untrusted data;
- self-signed third-party packages do not prove who the human owner is;
- key rotation and device revocation are recorded as signed events;
- UI must display signer, trust path and verification result.

### 8.5 Open format strategy

The format and basic reader should be open source.

Reasons:

- trust;
- user ownership;
- survivability if the company shuts down;
- integrations;
- community adapters;
- easier standard adoption;
- defensibility through ecosystem rather than lock-in.

The business does not depend on trapping data. It depends on operating the best runtime, sync, evaluation, governance and developer ecosystem around the format.

---

## 9. Local-First Storage

### 9.1 Source of truth

The canonical source of truth is:

```text
append-only events + cryptographic hash chains
```

SQLite tables are materialized projections optimized for use.

### 9.2 Local database

```text
soul.db
├── events
├── entities
├── evidence
├── provenance
├── relations
├── candidates
├── policy_rules
├── policy_decisions
├── decisions
├── embeddings
├── fts_entities
├── sync_heads
├── evaluations
└── audit_log
```

Recommended settings:

- encrypted SQLite;
- WAL mode;
- foreign keys enabled;
- prepared statements;
- short transactions;
- background checkpoints;
- periodic snapshots;
- integrity check after abnormal shutdown;
- frontend access only through a narrow Rust command API.

### 9.3 Event shape

```ts
interface SoulEvent {
  eventId: string;
  soulId: string;
  deviceId: string;
  actor: "user" | "importer" | "agent" | "system";
  hlc: string;
  operation:
    | "candidate.proposed"
    | "entity.activated"
    | "entity.updated"
    | "entity.superseded"
    | "entity.rejected"
    | "entity.deleted";
  entityType: SoulEntityType;
  entityId: string;
  payload: unknown;
  provenanceIds: string[];
  previousDeviceHash: string | null;
  contentHash: string;
  signature: string;
}
```

### 9.4 Why event sourcing is justified here

Event sourcing is normally too much complexity for an MVP. Here it directly solves product requirements:

- rollback;
- provenance;
- conflict visibility;
- auditability;
- device sync;
- personal policy history;
- tamper detection;
- evaluation reproducibility.

The implementation must remain narrow. SOUL does not need a generic event framework.

---

## 10. Ingestion and Memory Quality

### 10.1 MVP sources

The first release supports only:

- ChatGPT export;
- Claude export if available in a stable format;
- Markdown;
- JSON;
- manual entries;
- current SOUL conversations;
- explicit decisions from the UI.

The MVP excludes silent email, browser, microphone and desktop surveillance.

### 10.2 Pipeline

```text
Capture
→ Normalize
→ Segment
→ Exact parsers
→ Sensitivity classification
→ Candidate extraction
→ Provenance attachment
→ Deduplication
→ Contradiction detection
→ Activation policy
→ Candidate inbox
```

### 10.3 Cheap-first extraction

The extraction router must use this order:

1. exact parser for known export fields;
2. regex and deterministic patterns;
3. dictionary/entity matching;
4. local small classifier;
5. fast cloud model after redaction;
6. heavy model only for ambiguous high-value decisions.

### 10.4 Candidate scoring

```text
activation_score =
  0.30 * explicitness
+ 0.20 * source_trust
+ 0.15 * repetition
+ 0.15 * extraction_confidence
+ 0.10 * future_utility
- 0.10 * sensitivity
- 0.20 * contradiction_risk
```

The score sorts the inbox. It never bypasses hard confirmation rules.

### 10.5 Contradictions

Examples:

```text
2025: "I prefer React."
2026: "For small products I now prefer Svelte."
```

SOUL must not collapse these into a single timeless preference.

It should store:

- context;
- date;
- project scope;
- evidence;
- confidence;
- whether the new preference supersedes or only narrows the old one.

### 10.6 Memory poisoning defense

Imported content is always data, never authority.

Rules:

- external text cannot create a policy;
- agent output cannot create a confirmed boundary;
- facts cannot silently become instructions;
- instruction-like imported text is quarantined;
- all transformations retain provenance;
- sensitive candidates require confirmation;
- all changes can be rolled back.

Implementation controls:

- wrap imported content in an explicit untrusted-data field, never the instruction channel;
- use extraction-only prompts with no tools;
- require strict structured output and schema validation;
- reject unknown fields and instruction-shaped values in policy-related types;
- propagate a taint label from source through candidates and compiled context;
- prevent untrusted evidence from increasing authority without user confirmation;
- run deterministic checks before and after model extraction;
- test every importer against a prompt-injection corpus;
- never use imported text as a system prompt or policy DSL fragment;
- separate extraction models from action-authorizing models.

---

## 11. Retrieval and Context Compiler

### 11.1 Objective

The purpose is not to retrieve the maximum amount of context. It is to disclose the minimum context that materially improves the task.

```text
More memory is not automatically better.
Relevant, current and authorized context is better.
```

### 11.2 Retrieval order

```text
1. Classify task type
2. Apply policy prefilter
3. Exact typed lookup
4. FTS5/BM25
5. Vector similarity
6. Relation expansion
7. Temporal filtering
8. Conflict filtering
9. Reranking
10. Diversity and token packing
```

### 11.3 Ranking

```text
score =
  0.28 * typed_match
+ 0.22 * lexical_score
+ 0.18 * semantic_score
+ 0.12 * recency
+ 0.10 * importance
+ 0.10 * evidence_quality
+ boundary_boost
+ decision_scope_boost
- contradiction_penalty
- sensitivity_penalty
```

Weights are initial defaults and must be tuned against actual Soul Tests.

### 11.4 Context frame

```ts
interface SoulContextFrame {
  task: TaskDescriptor;
  hardConstraints: CompiledBoundary[];
  applicablePolicies: CompiledPolicy[];
  relevantDecisions: CompiledDecision[];
  preferences: CompiledPreference[];
  memories: CompiledMemory[];
  conflicts: CompiledConflict[];
  provenanceSummary: ProvenanceSummary;
  stateVersion: string;
  policyVersion: string;
}
```

### 11.5 Compilation priority

1. Hard boundaries.
2. Applicable policies.
3. Relevant prior decisions.
4. Output and communication preferences.
5. Necessary facts.
6. Uncertainty and conflicts.
7. Compact provenance markers.

### 11.6 Provider rendering

```text
[SOUL CONSTRAINTS]
- Never send external messages without confirmation. [bnd_17]

[RELEVANT DECISIONS]
- Use PostgreSQL for the MVP because operational simplicity matters more than theoretical scale. [dec_42]

[PREFERENCES]
- Prefer concise technical answers without emojis. [pref_9]

[UNCERTAINTY]
- Deployment region preference is inferred, not confirmed. [cand_31]
```

### 11.7 Context minimization rules

- Never send the entire `.soul`.
- Never send raw source documents by default.
- Do not expand provenance unless requested.
- Replace repeated evidence with one compact claim and IDs.
- Remove superseded entries.
- Keep contradictions explicit.
- Separate user facts from instructions.
- Enforce a hard token budget.
- Log exactly which entity IDs were disclosed.

---

## 12. Soul Policy Engine

### 12.1 Core idea

Most decisions do not require a reasoning model.

```text
"Amount over $500 requires approval"
```

is a deterministic policy, not an LLM problem.

### 12.2 Enforcement points

The Policy Engine runs:

1. before retrieval;
2. before memory disclosure;
3. before model invocation;
4. before tool call;
5. before tool result reaches a model;
6. before a memory write;
7. before an external side effect;
8. after a tool call to verify the effect;
9. before sync or export.

### 12.3 Effect lattice

```text
deny
> require_confirmation
> redact_or_transform
> allow
> abstain
```

`deny` overrides `allow`. `abstain` never means implicit permission for high-risk actions.

### 12.4 Tiered engine

| Tier | Mechanism | Target share | Model cost |
|---|---|---:|---:|
| T0 | OS permissions, sandbox, immutable invariants | 100% | $0 |
| T1 | Typed SoulRule DSL | 75-85% | $0 |
| T2 | Deterministic risk scorer and state machine | 10-18% | $0 |
| T3 | Small local or cheap structured-output model | 2-6% | Very low |
| T4 | Heavy reasoning model | Less than 2% | Controlled |

### 12.5 SoulRule example

```json
{
  "id": "policy_large_purchase",
  "priority": 900,
  "when": {
    "all": [
      { "eq": ["action.kind", "purchase.create"] },
      { "gt": ["action.amount", 500] }
    ]
  },
  "effect": "require_confirmation",
  "message": "Purchases above $500 require confirmation."
}
```

### 12.6 Allowed DSL operations

- `eq`, `neq`, `in`;
- numeric comparisons;
- `all`, `any`, `not`;
- typed action fields;
- time windows;
- recipient and domain matching;
- data sensitivity levels;
- amount and currency;
- user-presence freshness;
- action reversibility;
- environment and project scope.

### 12.7 Forbidden DSL capabilities

- dynamic evaluation;
- arbitrary shell execution;
- arbitrary JavaScript;
- network calls;
- unbounded regular expressions;
- hidden model invocation;
- side effects during evaluation.

### 12.8 Structured action schema

```ts
interface SoulAction {
  actionId: string;
  kind: string;
  actor: string;
  connectorId: string;
  accountId: string;
  environment: "development" | "staging" | "production";
  recipient?: string;
  domain?: string;
  amount?: number;
  currency?: string;
  dataClasses: string[];
  reversible: boolean;
  confirmedByUser: boolean;
  requestedScopes: string[];
  payloadHash: string;
}
```

### 12.9 Mandatory SOUL Gateway

A cooperative `soul_authorize_action` call is not enforcement. An agent can bypass a separate policy check if it still owns the underlying API key or can call the tool directly.

For a workflow to be described as governed by SOUL:

```text
Agent must not hold the destination credential.
Agent calls a SOUL-managed connector.
Gateway authorizes and executes the action atomically.
Connector verifies the result through a domain-specific readback contract.
```

Architecture:

```text
Agent
→ SOUL Gateway
→ policy evaluation
→ human confirmation if required
→ one-time execution capability
→ managed connector
→ destination API
→ trusted readback
→ signed receipt
```

An execution capability contains:

```ts
interface ExecutionCapability {
  capabilityId: string;
  actionId: string;
  connectorId: string;
  accountId: string;
  environment: string;
  payloadHash: string;
  policyVersion: string;
  nonce: string;
  issuedAt: string;
  expiresAt: string;
  maxUses: 1;
  signature: string;
}
```

Rules:

- one-time use;
- short expiration;
- atomic consume before execution;
- exact payload binding;
- connector, account and environment binding;
- replay detection;
- idempotency key for retry-safe connectors;
- credentials remain inside the connector or customer-controlled secret store;
- an unintegrated tool is labeled `observed`, not `enforced`.

The product must never claim to make every agent obey SOUL universally. It can enforce actions only when the execution path is mediated by SOUL Gateway, OS-level controls or another binding integration.

### 12.10 Escalation logic

```text
Typed action
→ hard invariant check
→ deterministic policy
→ deterministic risk score
→ cached precedent
→ small semantic classifier
→ heavy reasoner if unresolved
→ human gate for high impact
```

### 12.11 Human autonomy levels

```text
L0: Suggest only
L1: Draft, never send
L2: Act after explicit approval
L3: Act automatically within defined limits
L4: Fully autonomous for a narrow reversible domain
```

Default:

- new integrations start at L0 or L1;
- external communication starts at L1;
- low-risk reversible workflows can graduate to L3;
- finance, legal, public publishing and deletion never silently graduate;
- L4 requires explicit domain-specific activation.

### 12.12 Post-action verification

Authorization is not enough. SOUL must verify the result.

Example:

```text
Intent: cancel subscription
Authorized action: cancellation request
Tool result: HTTP 200
Verified effect: subscription status is still active
Final status: FAILED, not completed
```

This prevents agents from reporting success based only on a plausible tool response.

Verification is connector-specific. A connector declares:

- supported action kinds;
- idempotency behavior;
- authoritative readback source;
- expected state transition;
- verification timeout;
- rollback support;
- `unsupported` behavior when no trusted verification exists.

SOUL must return `authorized_but_unverified`, not `completed`, when a connector cannot independently verify the effect.

### 12.13 Override learning

```text
User overrides decision
→ ask one optional reason
→ create candidate policy or preference
→ replay it against similar past decisions
→ show expected changes
→ user confirms
→ policy version increments
```

No single override silently rewrites the user's identity.

---

## 13. Speed Architecture

### 13.1 Latency targets

| Stage | p50 | p95 |
|---|---:|---:|
| T0/T1 policy | 1 ms | 4 ms |
| Typed SQLite lookup | 2 ms | 8 ms |
| FTS retrieval | 3 ms | 15 ms |
| Vector retrieval | 5 ms | 25 ms |
| Reranking and packing | 3 ms | 12 ms |
| Context compiler | 5 ms | 20 ms |
| Full local hot path | 20 ms | 75 ms |
| Small local model | 100 ms | 700 ms |
| Cloud fast-model TTFT | 300 ms | 1.5 s |
| Heavy reasoning | 1.5 s | 8 s |

### 13.2 Work that must never block interaction

- old conversation ingestion;
- embedding generation for old entities;
- compaction;
- sync;
- candidate extraction backlog;
- analytics upload;
- benchmark aggregation;
- non-critical summaries.

These jobs run in a cancellable background queue.

### 13.3 Hot path

```text
Request
→ parse known fields
→ deterministic policy
→ exact/FTS retrieval
→ compile context
→ return or call fast model
```

### 13.4 Cold path

```text
Ambiguous request
→ small classifier
→ hybrid retrieval
→ conflict resolution
→ optional heavy reasoning
→ human confirmation if high risk
```

### 13.5 Performance principles

- Use Rust for storage, crypto, policy evaluation and indexing.
- Use TypeScript for product logic, integrations and UI.
- Keep the database local.
- Avoid network calls for policy checks.
- Avoid embeddings when exact or lexical retrieval is sufficient.
- Precompile policies.
- Incrementally update summaries.
- Cache by state and policy version.
- Stream model output after authorization is complete.
- Cancel stale requests immediately.

---

## 14. Token and Cost Optimization

### 14.1 Core rule

> The default SOUL operation must cost zero model tokens.

The majority of product behavior should be handled by:

- typed state;
- deterministic rules;
- exact search;
- FTS;
- cached embeddings;
- local classifiers;
- compact templates;
- user-confirmed precedents.

### 14.2 Token budgets

| Operation | Input tokens | Output tokens |
|---|---:|---:|
| Intent classification | 100-300 | 80 max |
| Candidate extraction | 800-2,000 | 100-400 |
| Default context frame | 400-900 | None |
| Complex context frame | 1,500 max | None |
| Absolute context cap | 3,000 | None |
| T3 policy classification | 500 max | 100 max |
| T4 reasoning | 2,000-6,000 | 300-1,500 |
| Automated eval judge | 2,500 max | 300 max |

### 14.3 Target routing distribution

```text
75-85% deterministic rules and exact state
10-18% retrieval and deterministic precedents
2-6% small local or cheap model
less than 2% heavy reasoning
```

### 14.4 Extraction batching

Do not call a model per message.

Recommended approach:

```text
Parse conversation export
→ deterministic candidate filtering
→ group 10-30 related messages
→ one structured extraction call
→ deduplicate before storage
```

### 14.5 Incremental processing

Every source chunk has a content hash.

If the source hash is unchanged:

- do not re-extract;
- do not re-embed;
- do not re-summarize;
- reuse candidate and provenance results.

### 14.6 Prompt architecture

All model calls must use compact structured prompts.

Bad:

```text
Here is the complete history of this user. Think deeply about who they are...
```

Good:

```text
TASK: classify candidate entity
ALLOWED TYPES: preference | decision | boundary | none
SOURCE TRUST: imported conversation
OUTPUT: strict JSON
SOURCE: ...
```

### 14.7 Context templates

Preferences should be rendered from typed templates rather than repeatedly summarized by a model.

```text
Preference: concise technical output
Scope: coding
Strength: 0.92
Exception: architectural decision records may be detailed
```

can render directly as:

```text
Use concise technical output. Detailed explanation is acceptable for architecture decisions.
```

### 14.8 Model routing

Models are selected by capability, not brand.

```ts
interface ModelCapability {
  provider: string;
  model: string;
  locality: "device" | "private_cloud" | "public_cloud";
  structuredOutput: boolean;
  reasoningClass: "none" | "light" | "heavy";
  privacyClass: number;
  latencyClass: number;
  costClass: number;
}
```

Routing objective:

```text
expected accuracy gain
- privacy cost
- latency cost
- monetary cost
- data exposure cost
```

### 14.9 BYOK and included usage

MVP:

- BYOK for power users;
- one optional managed fast-model route;
- local embeddings;
- no unlimited heavy inference;
- visible per-operation token and cost meter in developer mode.

Paid consumer plans can include a fair-use monthly fast-model allowance. Heavy reasoning is either BYOK, metered or limited by credits.

### 14.10 Caching

| Cache | Key |
|---|---|
| Embedding | model ID + normalized content hash |
| Retrieval | query fingerprint + state version + scope |
| Context frame | task signature + state version + policy version |
| Policy result | action hash + policy version + environment hash |
| Entity summary | entity ID + evidence head hash |
| Model classification | prompt hash + model revision |

Invalidation uses dependencies and versions, not only TTL.

### 14.11 Cost guardrails

- per-user daily inference ceiling;
- per-operation max input and output tokens;
- automatic fallback to BYOK;
- no retry with a more expensive model unless expected value is clear;
- one heavy reasoning call maximum before human escalation;
- spend alerts at 50%, 80% and 100%;
- provider outage fallback;
- no raw personal prompts in shared semantic caches;
- no model call inside deterministic policy evaluation.

---

## 15. Accuracy Optimization

### 15.1 Accuracy hierarchy

SOUL should prefer:

```text
Explicit user boundary
> confirmed decision
> confirmed preference
> repeated evidence
> inferred candidate
> generic model prior
```

### 15.2 Abstention

The system must be rewarded for saying:

```text
SOUL does not know enough to predict this decision.
```

False certainty is more damaging than a missing prediction.

### 15.3 Confidence calibration

Confidence must be trained against outcomes and user overrides, not generated as arbitrary model prose.

Calibration buckets:

| Reported confidence | Target observed correctness |
|---|---:|
| 50-60% | Approximately 55% |
| 60-70% | Approximately 65% |
| 70-80% | Approximately 75% |
| 80-90% | Approximately 85% |
| 90-100% | Approximately 95% |

Use reliability diagrams and Brier score.

### 15.4 Decision precedents

Past decisions are stronger than vague preferences.

For each new decision, retrieve:

- similar decisions;
- reasons that mattered;
- rejected alternatives;
- conditions that would have changed the choice;
- subsequent outcome;
- whether the user later regretted or reversed it.

### 15.5 Contradiction-aware output

When two principles conflict, SOUL must expose the conflict:

```text
Principle A: preserve runway
Principle B: move quickly when distribution is available

The recommendation depends on whether the offer includes measurable distribution.
```

### 15.6 Eval-driven development

Every retrieval, ranking, extraction and prompt change is tested against:

- blind preference win rate;
- boundary violation rate;
- factual contradiction rate;
- token cost;
- latency;
- abstention quality;
- user edit distance.

No optimization ships solely because outputs "feel better."

### 15.7 Golden datasets

Maintain:

- synthetic identity fixtures;
- anonymized opt-in user fixtures;
- contradiction cases;
- stale-memory cases;
- malicious prompt-injection cases;
- boundary conflicts;
- policy bypass attempts;
- ambiguous decision cases;
- cross-model consistency cases.

---

## 16. Privacy and Security

### 16.1 Security position

SOUL may contain a person's most sensitive digital model. Privacy is not a settings page. It is the architecture.

### 16.2 Principles

- Local-first source of truth.
- End-to-end encrypted optional sync.
- User-held keys.
- Purpose-bound context requests.
- Minimal disclosure.
- Explicit access receipts.
- No plaintext server retrieval.
- Full export and deletion.
- Open format.
- Signed updates.
- Least-privilege integrations.

### 16.3 Threat model

| Threat | Primary control |
|---|---|
| Stolen device | Encrypted DB, OS keychain, auto-lock |
| Malicious import | Untrusted provenance, no executable package code |
| Agent asks for whole SOUL | Purpose-bound scoped context API |
| MCP server exfiltrates data | Per-server scopes, egress policy, redaction |
| Cloud operator sees sync | E2EE opaque events |
| Cloud model receives secrets | Local routing, redaction, explicit consent |
| Data rollback or tampering | Signed hash chains and snapshots |
| Malicious application update | Signed builds and pinned channels |
| Supply-chain compromise | Minimal dependencies, lockfiles, SBOM |
| Shared computer access | Profile isolation and OS unlock |
| Sensitive inferred memory | Confirmation-only activation |
| Deleted policy returns after sync | Signed tombstones and key epochs |

### 16.4 Keys

- local master key generated on device;
- master key stored through OS keychain or Stronghold;
- separate keys derived for database, blobs, sync and package export;
- device identity uses Ed25519;
- new device pairing through QR or short authentication string;
- revocation rotates the key epoch;
- revoked devices cannot decrypt future events.

#### Recovery model

During setup, the user chooses one of three explicit recovery modes:

```text
Maximum privacy
Recovery key only. Losing every trusted device and the recovery key means permanent loss.

Trusted recovery
Recovery key plus two approved recovery contacts or devices.

Managed recovery
Encrypted key escrow split across user authentication and a customer-held recovery secret.
```

The product must explain the tradeoff before activation. Recovery material is tested through an optional recovery drill. SOUL never imply that server-blind encryption and effortless provider recovery are simultaneously possible without a user-held or delegated recovery factor.

#### Deletion semantics

"Delete" has defined stages:

1. remove entities from active projections;
2. delete associated plaintext blobs;
3. compact local history into a new snapshot that excludes deleted content;
4. rotate encryption keys when historical ciphertext could reveal deleted content;
5. send server purge requests for encrypted events and blobs;
6. expire backup copies under a published retention window;
7. retain only legally required, non-content billing or security metadata;
8. show the user a deletion receipt and remaining retention deadline.

Signed tombstones alone do not equal erasure. Product copy must distinguish `removed from active state`, `cryptographically erased` and `purged from backups`.

### 16.5 Purpose-bound context request

```json
{
  "purpose": "draft_project_status",
  "recipient": "internal_team",
  "maxSensitivity": "private",
  "tokenBudget": 700,
  "expiresInSeconds": 600
}
```

The requester receives a compiled context, not direct database access.

### 16.6 Access receipts

Every external request records:

- requester;
- purpose;
- scopes;
- entity IDs disclosed;
- redactions;
- policy version;
- timestamp;
- expiration;
- result;
- user approval if required.

### 16.7 Legal posture

Early product claims must avoid:

- medical diagnosis;
- legal decision automation;
- financial fiduciary claims;
- consciousness claims;
- guaranteed identity accuracy;
- compliance certification without audit;
- fully autonomous irreversible actions.

SOUL is decision-support and authority infrastructure. The user remains the principal.

---

## 17. Sync Architecture

### 17.1 MVP rule

Sync is not required for the first fundraising demo. The first MVP can be fully local.

### 17.2 Beta sync

The server stores only:

```text
user_id
device_id
event_id
encrypted_event
event_hash
causal_heads
key_epoch
created_at
```

### 17.3 Sync flow

1. Client sends known heads.
2. Server returns missing opaque events.
3. Client verifies signatures and hash chains.
4. Client applies events in causal order.
5. Local projections update.
6. Conflicts enter a user-visible queue.

Rollback and fork detection:

- every trusted device persists the latest accepted signed heads;
- devices compare heads during pairing and sync;
- a server response older than a trusted head is rejected;
- inconsistent valid chains create a visible fork incident;
- new devices receive a checkpoint signed by an existing trusted device or recovery root;
- high-assurance enterprise deployments may anchor periodic checkpoint hashes in an independent transparency service.

Hash chains detect mutation inside a presented chain. Trusted heads and cross-device comparison are required to detect a server withholding newer valid events.

### 17.4 Merge rules

| Data | Merge behavior |
|---|---|
| Tags | OR-set |
| Evidence links | Grow-only plus tombstones |
| Low-risk metadata | HLC last-write-wins |
| Fact value | Multi-value conflict |
| Preference | Merge evidence, do not overwrite blindly |
| Boundary | Never weaken automatically |
| Policy | Conflict requires confirmation |
| Delete | Signed tombstone |
| Decision | Immutable; new decision supersedes old |

### 17.5 Backend

Recommended first backend:

- Supabase Auth;
- Postgres for encrypted events and device metadata;
- RLS on every exposed table;
- Realtime only as a notification mechanism;
- Storage for encrypted blobs;
- no server-side decryption;
- no server-side vector search over personal plaintext.

---

## 18. API and MCP

### 18.1 Local API

```text
soul.context.compile
soul.policy.evaluate
soul.action.authorize
soul.action.verify

soul.memory.propose
soul.memory.approve
soul.memory.reject
soul.memory.search
soul.memory.explain

soul.decision.record
soul.boundary.set
soul.policy.upsert

soul.test.create
soul.test.submit
soul.test.results

soul.sync.status
soul.sync.run
soul.export
```

### 18.2 MCP tools

```text
soul_compile_context
soul_authorize_action
soul_verify_action
soul_propose_memory
soul_record_decision
soul_explain_decision
soul_start_test
```

### 18.3 Scopes

```text
soul:context:read
soul:memory:propose
soul:memory:approve
soul:policy:evaluate
soul:policy:write
soul:action:authorize
soul:test:run
```

### 18.4 Transport

- Unix domain socket on macOS/Linux;
- named pipe on Windows;
- loopback HTTP only when required;
- short-lived capability tokens;
- client identity binding;
- no open unauthenticated local port;
- explicit user approval for new clients.

### 18.5 MCP compatibility strategy

- support the stable MCP revision used by major clients;
- isolate protocol transport from SOUL domain logic;
- use capability negotiation;
- request minimal OAuth scopes;
- never pass through third-party tokens;
- store state in SOUL, not in MCP sessions;
- use explicit resource identifiers and audience validation for remote servers.

---

## 19. Technical Stack

### 19.1 Monorepo

```text
apps/
  desktop/
  web/
  worker/

packages/
  soul-schema/
  soul-core/
  soul-policy/
  soul-context/
  soul-evals/
  soul-mcp/
  shared-ui/

crates/
  soul-storage/
  soul-crypto/
  soul-policy-runtime/
  soul-desktop/
```

### 19.2 Desktop

- Tauri 2;
- React;
- TypeScript;
- Vite;
- Tailwind CSS;
- Rust for storage, crypto, indexing and policy evaluation;
- Zod and JSON Schema at boundaries;
- minimal state management;
- no remote web content inside privileged Tauri windows.

### 19.3 Local data

- encrypted SQLite;
- FTS5;
- optional vector adapter;
- local embeddings;
- OS keychain or Stronghold;
- Ed25519;
- XChaCha20-Poly1305;
- content hashes and append-only events.

### 19.4 Web

- Next.js App Router;
- TypeScript;
- Tailwind CSS;
- product site;
- account and device management;
- public anonymized Soul Test cards;
- no plaintext personal SOUL in the web app.

### 19.5 Backend

- Supabase Auth;
- Supabase Postgres;
- RLS;
- Supabase Storage for encrypted blobs;
- optional Realtime notifications;
- pgTAP for database and RLS tests.

### 19.6 Edge and model broker

- Cloudflare Worker or Hono;
- provider routing;
- retry and fallback;
- spend limits;
- rate limiting;
- PII-free metadata;
- no sensitive prompt logging by default;
- no cloud dependency for local policy checks.

---

## 20. 14-Day Fundraising Prototype

### 20.1 Objective

Build the smallest product that demonstrates:

1. user-owned structured identity;
2. measurable personalization;
3. portability across existing AI clients;
4. a believable path to agent governance.

This is an investable product prototype, not a production security boundary. Security and governance claims remain limited until the mandatory Gateway, credential isolation, replay protection and connector-specific verification are implemented.

### 20.2 Included

- Tauri desktop app;
- create local `.soul`;
- five-minute guided calibration without external data;
- local background runtime and control center with no chat surface;
- one-click MCP connection for supported coding clients;
- Chromium Browser Companion for ChatGPT Web, Gemini Web and Claude Web;
- secure Native Messaging bridge between Browser Companion and the local runtime;
- five entity types: fact, preference, decision, boundary, goal;
- candidate inbox;
- provenance preview;
- local SQLite;
- basic FTS retrieval;
- context compiler;
- connection to at least two existing AI clients through client adapters;
- blind A/B test runner;
- B1 baseline built from a short manually reviewed profile;
- Soul Win Rate report;
- shareable privacy-safe result card;
- full local export and delete;
- one deterministic boundary demo.

### 20.3 Excluded

- automatic desktop capture;
- arbitrary browser automation outside explicitly supported AI chat domains;
- silent installation of a browser extension;
- email and calendar ingestion;
- mobile app;
- family mode;
- social network;
- marketplace;
- full sync;
- autonomous actions;
- complex graph UI;
- own foundation model;
- enterprise dashboard;
- billing beyond a waitlist or simple founder plan.

### 20.4 Day-by-day plan

#### Days 1-2: contracts

- entity schemas;
- event schema;
- SQLite migration;
- `.soul` manifest;
- extraction JSON schema;
- blind test record format;
- threat model for MVP.

#### Days 3-4: calibration and activation

- rapid preference choices;
- short explicit questions;
- optional writing samples;
- typed initial state with explicit provenance;
- compact review and activation;
- resumable progress.

#### Days 5-6: runtime and review UX

- candidate inbox;
- approve, edit, reject;
- provenance display;
- local profile summary;
- local background service lifecycle;
- connection status and disclosure receipts;
- delete and export.

#### Days 7-8: retrieval

- exact lookup;
- FTS5;
- relevance ranking;
- compact context compiler;
- token count preview.

#### Days 9-10: client portability

- MCP adapter for supported coding clients;
- one-click configuration with backup and rollback;
- Browser Companion for ChatGPT Web and Gemini Web on Chromium browsers;
- Native Messaging pairing with the desktop runtime;
- allowlisted, versioned site adapters for supported web chats;
- single-click send interception, local context compilation and automatic resume;
- collapsed `SOUL context: N items` disclosure inside the rendered chat;
- same domain context delivered through MCP and Browser Companion;
- context disclosure receipt.

#### Days 11-12: Blind Soul Test

- randomized variants;
- `Neither` option;
- hidden assignments;
- result storage;
- Wilson interval;
- share card.

#### Day 13: policy demo

- typed action schema;
- one spending rule;
- one external-message rule;
- allow, deny and confirmation UI;
- audit receipt.

#### Day 14: launch package

- polished onboarding;
- 45-second demo video;
- landing page;
- public methodology;
- founder story;
- waitlist and feedback;
- investor one-pager;
- bug and privacy pass.

#### Days 15-28: validation window

- onboard 20 external users;
- measure calibration completion and first connected-client task;
- collect 20-round and extended tests;
- observe seven-day retention;
- test $99 and $240 annual willingness to pay;
- interview users who reject candidates;
- compare SOUL against B1;
- decide whether to continue the consumer wedge.

### 20.5 Demo script

```text
0-5 sec
"I already work in Codex, Claude and ChatGPT. I should not need another chat window."

5-12 sec
Create a useful local SOUL through rapid calibration and click Connect to Codex.

12-20 sec
The user types a normal task in Codex. Codex retrieves a minimal context frame from the local SOUL.

20-32 sec
SOUL shows a disclosure receipt, while the work and answer remain inside Codex.

32-40 sec
Open a normal ChatGPT or Gemini web chat. The SOUL badge appears beside the composer and the next prompt receives the same identity automatically.

40-48 sec
An agent attempts a $600 action. Local policy blocks it in milliseconds.

48-55 sec
"The intelligence changed. The person and authority did not."
```

### 20.6 MVP success criteria

The prototype plus the following two-week validation window are successful if:

- 20 external users complete calibration without requiring an import;
- median time to a useful initial SOUL is under 5 minutes;
- at least 15 users complete a first SOUL-assisted task in an existing AI client;
- at least 15 users complete 20 blind rounds;
- pooled SOUL win rate beats the strongest simple baseline;
- at least 40% of activated users return within seven days;
- at least 5 users connect a second AI client;
- candidate approval rate exceeds 65%;
- no sensitive candidate is activated without confirmation;
- median compiled context stays below 900 tokens;
- local policy p95 stays below 10 ms for the demo rules;
- at least 3 users ask to pay or request continued access.

---

## 21. 90-Day Product Beta

### Phase 1: local core, weeks 1-4

- encrypted SQLite;
- event log;
- typed entities;
- provenance;
- import and export;
- candidate inbox;
- basic blind tests;
- two model providers.

### Phase 2: context quality, weeks 5-7

- hybrid retrieval;
- local embeddings;
- contradiction handling;
- decision precedent retrieval;
- token packing;
- cache invalidation;
- eval dashboard.

### Phase 3: governance, weeks 8-10

- SoulRule DSL;
- T0-T2 engine;
- action authorization;
- confirmation UX;
- audit receipts;
- MCP server;
- tool pre-check and post-check.

### Phase 4: paid beta, weeks 11-13

- encrypted sync;
- billing;
- device management;
- usage and cost controls;
- founder plan;
- developer SDK;
- public benchmark methodology;
- security hardening.

### 90-day targets

- 1,000 installs;
- 300 activated SOULs;
- 150 completed blind tests;
- 100 users connected to two or more models;
- 50 weekly active power users;
- 20 paying users;
- 5 developer integrations;
- 3 design partners using governance;
- less than 900 median context tokens;
- more than 85% policy decisions without heavy reasoning;
- less than 2% false-deny rate in defined low-risk workflows;
- zero silent high-risk policy bypasses.

---

## 22. Business Model

### 22.1 Strategy

Use open-core plus paid convenience and governance.

Free and open:

- `.soul` format;
- local runtime basics;
- local storage;
- import and export;
- basic context compilation;
- basic policy evaluation;
- developer protocol.

Paid:

- encrypted sync;
- automatic ingestion;
- advanced evals;
- multi-device management;
- managed model routing;
- agent governance;
- action receipts;
- policy versioning;
- team controls;
- hosted developer infrastructure;
- enterprise deployment and support.

### 22.2 Consumer pricing

#### SOUL Local

**$0 forever**

- one local SOUL;
- manual and file imports;
- local typed memory;
- local search;
- basic Blind Soul Test;
- one connected model at a time;
- BYOK;
- basic context compiler;
- basic boundaries;
- full export and delete;
- community support.

The user must never pay to own or export their identity.

#### SOUL Plus

**$12/month or $99/year**

- encrypted multi-device sync;
- unlimited connected model profiles;
- automatic import updates;
- advanced blind tests;
- decision history and rollback;
- context branches such as Work, Personal and Public;
- managed fast-model allowance;
- weekly calibration report;
- priority candidate extraction;
- email support.

#### SOUL Operator

**$29/month or $240/year**

- everything in Plus;
- Guardian mode;
- agent authorization;
- advanced policies;
- MCP and local API;
- action receipts;
- policy replay;
- automatic low-risk actions within limits;
- developer mode and cost telemetry;
- local model routing;
- higher managed inference allowance.

### 22.3 Developer pricing

#### Developer

**$49/month**

- one application;
- 10,000 governed context or policy operations;
- local SDK;
- testing environment;
- basic analytics;
- community support;
- BYOK inference.

#### Team

**$299/month**

- multiple applications;
- 100,000 governed operations;
- shared policy registry;
- signed audit exports;
- staging and production environments;
- role-based access;
- private support channel;
- usage analytics.

#### Business

**$1,500/month**

- up to 100 production agents;
- fleet policy management;
- SSO;
- longer evidence retention;
- approval workflows;
- custom policy packs;
- priority support;
- deployment assistance.

#### Enterprise

**From $30,000 to $100,000+ ARR**

- VPC or on-prem deployment;
- SCIM;
- SLA;
- custom retention;
- customer-managed keys;
- security review;
- custom integrations;
- dedicated support;
- contractual usage and liability boundaries.

### 22.4 Usage policy

- Local deterministic operations are unlimited.
- BYOK inference is not marked up in the local product.
- Managed fast-model usage has a fair-use allowance.
- Heavy reasoning uses credits or BYOK.
- Enterprise inference is passed through or billed separately.
- Never bundle unlimited expensive reasoning into a flat consumer plan.

### 22.5 Why these prices

The consumer anchor sits below or near major AI subscriptions while offering a cross-model layer rather than another model subscription.

The developer anchor is above basic memory infrastructure only when SOUL provides measurable governance, evaluation and audit value.

The annual discount improves cash flow and lowers payment-processing overhead:

```text
Plus annual: $99, equivalent to $8.25/month
Operator annual: $240, equivalent to $20/month
```

### 22.6 Recommended launch pricing

For the first 100 paying users:

```text
Founder Plus: $79/year locked for two years
Founder Operator: $179/year locked for two years
```

Do not sell a lifetime plan. Long-term encrypted sync and security updates create permanent costs.

Treat these prices as launch hypotheses, not doctrine. Test willingness to pay immediately after the user sees a completed Blind Soul Test:

```text
Control: join free beta
Variant A: reserve Plus at $99/year
Variant B: reserve Operator at $240/year
```

Measure checkout starts and refundable deposits rather than relying only on survey answers. Keep the product UI limited to Local, Plus and Operator until there is real B2B demand; the developer and enterprise tiers are the expansion model, not day-one pricing-page clutter.

---

## 23. Unit Economics

### 23.1 Consumer target economics

Approximate monthly target per active paid user:

| Cost item | Plus | Operator |
|---|---:|---:|
| Sync and storage | $0.25-$0.55 | $0.40-$0.90 |
| Managed fast-model usage | $0.35-$0.85 | $1.00-$2.50 |
| Observability and email | $0.05-$0.15 | $0.10-$0.25 |
| Payment processing allocation | $0.25-$0.65 | $0.35-$1.25 |
| Support allocation at scale | $0.20-$0.40 | $0.60-$1.40 |
| Target total COGS | $1.10-$2.60 | $2.45-$6.30 |

Target gross margin:

```text
SOUL Plus: 68-87% across the annual/monthly mix
SOUL Operator: 68-92% across the annual/monthly mix
Developer self-serve: 85-95% with BYOK
Enterprise software: 80-90% excluding implementation
```

If managed inference pushes a plan below 70% gross margin for two consecutive months, reduce the included allowance, route more work locally or move heavy reasoning to credits. Do not hide the problem inside an "unlimited" plan.

### 23.2 Margin protection

- local inference where useful;
- local embeddings;
- BYOK default for power users;
- hard context caps;
- cheap-first model routing;
- deterministic policy engine;
- asynchronous batch extraction;
- no repeated extraction of unchanged content;
- annual plans;
- separate professional services;
- no unlimited heavy reasoning.

### 23.3 Consumer scenario

Illustrative, not a forecast:

| Metric | Early | Strong |
|---|---:|---:|
| Monthly active users | 10,000 | 100,000 |
| Paid conversion | 3% | 6% |
| Paying users | 300 | 6,000 |
| Blended monthly ARPU | $15 | $17 |
| MRR | $4,500 | $102,000 |
| ARR | $54,000 | $1.224M |

Consumer revenue validates demand. The path to a very large company likely combines:

```text
consumer subscription
+ developer API
+ agent governance
+ enterprise control plane
```

### 23.4 B2B expansion

The business expands naturally as:

- one user connects more models;
- one developer adds more applications;
- one company governs more agents;
- more actions require evidence;
- policies become shared organizational assets;
- model switching increases the value of provider neutrality.

### 23.5 Acquisition and retention targets

Gross margin alone is not unit economics. The operating model must track:

| Metric | Consumer target | Developer/B2B target |
|---|---:|---:|
| Calibration-to-activation | More than 60% | More than 70% |
| Activated-to-paid | 3-8% | 10-25% after qualified trial |
| Monthly paid churn | Less than 4% | Less than 2% logo churn |
| Annual renewal | More than 65% | More than 80% |
| Refund rate | Less than 5% | Less than 2% |
| Organic/referral acquisition | More than 60% initially | More than 30% |
| CAC payback | Less than 6 months | Less than 12 months |
| Support time | Less than 10 minutes per paid user/month | Priced into contract |

Early consumer paid acquisition should remain near zero until week-4 retention and repeated use through connected clients are proven. A $99 annual subscription cannot support broad paid acquisition if first-year contribution margin is only $60-$75.

For B2B, implementation work is quoted separately. Founder time spent on custom deployment must not be hidden inside software gross margin.

---

## 24. Go-to-Market

### 24.1 Launch narrative

Do not launch as:

> Universal memory for AI.

Launch as:

> **I moved two years of ChatGPT context to another model, then tested my AI clone blindly.**

### 24.2 Launch content

Required assets:

1. 45-second product demo.
2. Founder blind-test video.
3. Public evaluation methodology.
4. Privacy architecture page.
5. Open `.soul` format repository.
6. Interactive demo with synthetic data.
7. Shareable Soul Test cards.
8. Direct comparison with a simple profile prompt.

### 24.3 Viral hooks

- "Can your AI tell you apart from everyone else?"
- "My clone beat generic AI in 37 of 48 blind decisions."
- "I switched the model. The person stayed the same."
- "ChatGPT and Claude disagree about who I am. SOUL makes the profile mine."
- "I gave five agents one constitution."

### 24.4 Distribution loops

#### Blind-test loop

```text
Complete test
→ receive private share card
→ friend runs own test
→ new SOUL created
```

#### Portability loop

```text
Connect second model
→ see immediate continuity
→ recommend SOUL to another multi-model user
```

#### Integration loop

```text
Developer publishes adapter
→ users of that tool install SOUL
→ more developers request support
```

#### Policy-pack loop

```text
User publishes safe policy template
→ others install it
→ template links back to SOUL
```

#### Benchmark loop

```text
Public benchmark improves
→ researchers and builders discuss methodology
→ SOUL becomes category reference
```

### 24.5 First communities

- AI power users;
- Claude Code and Codex users;
- local-first software communities;
- Obsidian and personal knowledge management power users;
- privacy-focused developers;
- agent framework builders;
- founders publicly building with multiple models.

### 24.6 Product-led sales motion

```text
Individual developer uses local SOUL
→ connects it to an agent
→ team needs shared governance
→ company buys Team
→ security requires enterprise controls
```

---

## 25. Investor Strategy

### 25.1 What investors must understand in ten seconds

```text
Every AI builds a different model of you.
SOUL makes that model yours, measures whether it works and enforces it wherever agent execution runs through SOUL Gateway.
```

### 25.2 What makes the MVP fundable

An investor should see:

- a real desktop product, not slides;
- immediate import;
- a visible `.soul` object;
- blind A/B proof;
- portability between existing AI clients;
- a local instant policy block;
- privacy-safe architecture;
- an open format;
- early users with completed tests;
- evidence that people want continued access.

### 25.3 What not to claim

- guaranteed digital immortality;
- conscious AI;
- complete clone of a human;
- perfect prediction;
- zero hallucinations;
- complete legal compliance;
- all applications supported;
- fully autonomous action from day one;
- a trillion-dollar TAM with no bottom-up argument.

### 25.4 YC-style answer

**What are you building?**

> SOUL is a local executable model of a user. It transfers their decisions and boundaries across AI products, measures personalization through blind tests, and controls what integrated agents can know and do through a binding execution gateway.

**Who needs it now?**

> People and developers using multiple AI models and agents. Their context is fragmented, impossible to verify and increasingly dangerous as agents take actions.

**What is the initial wedge?**

> Build a useful local SOUL in five minutes, connect it to the AI tools you already use and keep working inside those tools. SOUL supplies minimal relevant context through client integrations; optional history imports improve it later, while blind tests measure whether personalization beats a simple reviewed profile.

**Why is it not just memory?**

> Memory retrieves facts. SOUL stores typed decisions and boundaries, compiles minimal context, measures whether personalization works and enforces policies before agents act.

**Why cannot OpenAI copy it?**

> Any model provider can improve memory inside its own product. SOUL is valuable because it is user-owned, provider-neutral, measurable and controls external agents across vendors.

### 25.5 Fundraising proof ladder

#### Day-one proof

- working demo;
- founder blind-test result;
- 10-20 design users;
- open format;
- credible architecture;
- strong founder velocity.

#### Pre-seed proof

- 1,000 installs;
- 300 activated users;
- clear blind-test lift;
- 20 paying users or strong paid intent;
- 3 governance design partners;
- high-quality public benchmark.

#### Seed proof

- strong retention among multi-model power users;
- $250K-$500K ARR or exceptional growth;
- repeatable developer adoption;
- millions of governed actions;
- verified enterprise deployments;
- independent security review;
- expansion from personal context to agent governance.

---

## 26. North-Star Metrics

### 26.1 Primary product metric

```text
Weekly verified SOUL-assisted decisions or actions
```

This excludes passive storage and unverified model calls.

### 26.2 Activation

- guided calibration completed;
- initial local SOUL reviewed and activated;
- first AI client connected through MCP or Browser Companion;
- first task-specific context request completed in an external client;
- first disclosure receipt inspected or acknowledged.

### 26.3 Quality

- Soul Win Rate;
- candidate acceptance rate;
- memory correction rate;
- boundary violation rate;
- false-deny rate;
- confidence calibration;
- user edit distance;
- context tokens per successful task;
- percentage of actions handled without heavy reasoning.

### 26.4 Retention

- week-1 retained activated users;
- week-4 retained multi-model users;
- weekly context injections;
- weekly completed decisions;
- connected AI clients per user;
- active policies per user;
- number of overrides converted into confirmed rules.

### 26.5 Business

- free-to-paid conversion;
- annual-plan share;
- monthly inference COGS per paid user;
- gross margin;
- expansion from Plus to Operator;
- developer-to-team conversion;
- governed actions per paying workspace;
- net revenue retention for B2B.

### 26.6 Reliability

- local hot-path p95;
- policy p95;
- sync convergence time;
- audit completeness;
- context cap violations;
- policy bypass count;
- crash-free sessions;
- import failure rate.

---

## 27. Risk Register

### 27.1 "This is astrology"

**Risk:** users accept generic descriptions as personally accurate.

**Mitigation:** blind randomized A/B tests, strong baselines, `Neither`, placebo rounds, held-out questions and public statistical methodology.

**Stop condition:** if full SOUL cannot consistently beat a short profile prompt, simplify or change the product.

### 27.2 Portable memory competitors

**Risk:** Egoist Machines and similar products own the AI Passport category.

**Mitigation:** position SOUL around verified decision models and agent authority, not a passive passport. Support memory products as inputs rather than trying to replace all of them.

### 27.3 Platform copying

**Risk:** OpenAI, Anthropic or Google builds better native memory.

**Mitigation:** provider neutrality, open format, personal evals, policy enforcement, external agent integration and local ownership.

### 27.4 Cold start

**Risk:** an empty SOUL provides no value.

**Mitigation:** import existing history, extract high-value decisions first, run a useful short test within minutes and avoid requiring manual ontology design.

### 27.5 Hallucinated memories

**Risk:** SOUL records jokes, temporary frustration or model inference as identity.

**Mitigation:** typed candidates, provenance, confirmation for sensitive or important claims, expiration, contradiction handling and rollback.

### 27.6 Token cost explosion

**Risk:** every operation uses a heavy model and destroys margins.

**Mitigation:** deterministic fast path, local storage, exact retrieval, batching, local embeddings, cached structured extraction, hard caps, BYOK and heavy-model fallback below 2%.

### 27.7 Latency

**Risk:** SOUL makes every integrated agent feel slower.

**Mitigation:** policy in Rust, local retrieval, precompiled rules, asynchronous ingestion, streaming after authorization and strict p95 budgets.

### 27.8 Privacy breach

**Risk:** a `.soul` leak exposes years of personal context.

**Mitigation:** local-first encrypted storage, E2EE sync, purpose-bound disclosure, user-held keys, no plaintext server retrieval, audits and full deletion.

### 27.9 Memory poisoning

**Risk:** imported content or malicious agent output changes policies.

**Mitigation:** untrusted provenance, separation of facts and authority, confirmation for policies, no executable imports, signed event history and quarantine.

### 27.10 Wrong autonomous action

**Risk:** SOUL authorizes a damaging action based on an incorrect prediction.

**Mitigation:** autonomy levels, deny/confirm defaults for high impact, deterministic hard boundaries, explicit scopes, post-action verification and contractual limits.

### 27.11 User maintenance burden

**Risk:** the product becomes an Obsidian vault that requires constant gardening.

**Mitigation:** candidate inbox, batch approval, automatic expiration suggestions, focus on decisions and boundaries, and advanced graph views hidden from default UX.

### 27.12 No daily habit

**Risk:** users import once and never return.

**Mitigation:** SOUL operates through integrations, Decision Inbox, context requests, client portability, weekly evals and agent governance. Success is not measured by app opens alone.

### 27.13 Open source reduces monetization

**Risk:** competitors fork the runtime.

**Mitigation:** monetize encrypted sync, managed routing, distribution, trusted updates, evaluation network, governance control plane, enterprise deployment and developer ecosystem.

### 27.14 Seventeen-year-old founder credibility

**Risk:** investors or enterprise buyers doubt security and execution maturity.

**Mitigation:** public methodology, working product, exceptional shipping speed, precise claims, open threat model, advisors, independent security review and no fake enterprise theater.

---

## 28. Moat

### 28.1 Weak moats

These are not defensible alone:

- embeddings;
- vector search;
- chat import;
- a system prompt;
- a graph visualization;
- local SQLite;
- model routing;
- generic MCP support.

### 28.2 Stronger moat stack

#### Personal eval dataset

Each user accumulates blind choices and confirmed overrides that directly measure how their SOUL differs from generic model behavior.

The raw dataset belongs to the user and remains exportable. The company-level learning loop comes only from explicit opt-in aggregation of non-content metrics, anonymized benchmark tasks, model-routing outcomes and signed evaluation summaries. SOUL must not claim proprietary ownership of private user identity data as a moat.

#### Decision graph

Confirmed decisions include alternatives, reasons, outcomes and conditions that would change the decision.

#### Policy history

Boundaries and permissions become tested, versioned and replayable.

#### Context compiler

The system learns how to disclose minimum context for maximum lift.

#### Open format adoption

If `.soul` becomes the expected portable object, the company owns the leading runtime and ecosystem around the standard.

The format itself is not the moat. Commercial defensibility must come from:

- the best reference runtime;
- trusted signed releases;
- the largest adapter and connector network;
- the strongest public personalization benchmark;
- privacy-preserving aggregate routing data;
- hosted sync and governance reliability;
- enterprise policy distribution;
- brand trust.

#### Trust

Local-first architecture, transparent access receipts and reliable export are difficult to manufacture after a privacy failure.

#### Integration network

Every model, agent and application adapter increases the value of the user-owned runtime.

### 28.3 Compounding loop

```text
More verified decisions
→ better personalization
→ more agent delegation
→ more corrections and policies
→ stronger evals
→ higher trust
→ more integrations
→ more verified decisions
```

---

## 29. Product Roadmap

### Stage 1: verified identity

- import;
- typed model;
- blind tests;
- cross-model context;
- local ownership.

### Stage 2: decision runtime

- decision precedents;
- confidence calibration;
- branches;
- Decision Inbox;
- override learning.

### Stage 3: agent governance

- deterministic policies;
- MCP authorization;
- context receipts;
- autonomy levels;
- post-action verification.

### Stage 4: developer platform

- SDK;
- hosted control plane;
- adapter ecosystem;
- policy packs;
- evaluation API;
- usage analytics.

### Stage 5: organizational authority

- delegated roles;
- team policies;
- shared organizational context;
- agent fleets;
- enterprise audit;
- VPC and on-prem.

### Stage 6: standard

- open specification;
- independent implementations;
- identity and policy portability;
- signed Soul modules;
- trusted certification;
- user-controlled context exchange across applications.

---

## 30. Product Decisions to Freeze

The following decisions should remain fixed through the fundraising MVP:

1. Local-first source of truth.
2. Blind A/B proof is the hero feature.
3. Same-model comparison is mandatory.
4. `Neither` is mandatory.
5. Full export and deletion are free.
6. Heavy reasoning is fallback, not default.
7. Policies are declarative and deterministic.
8. High-risk actions require human confirmation.
9. The MVP supports one import format extremely well.
10. No graph UI as the primary interface.
11. No marketplace.
12. No digital-consciousness claims.
13. No silent surveillance.
14. No unlimited managed heavy inference.
15. No enterprise customization before the core eval works.

---

## 31. Kill Criteria and Pivots

### 31.1 Kill or redesign the consumer wedge if

- fewer than 20% of imported users complete a Blind Soul Test;
- users cannot perceive value without extensive explanation;
- full SOUL does not beat a simple profile prompt;
- candidate correction burden remains high;
- week-4 retention among multi-model users stays below 15%;
- users are unwilling to connect a second AI client;
- privacy concerns prevent import despite local processing.

### 31.2 Potential pivot A: eval infrastructure

If users value the blind methodology more than persistent identity:

> Build the standard evaluation platform for AI personalization.

### 31.3 Potential pivot B: agent governance

If developers value policies more than consumer identity:

> Build the provider-neutral authority and evidence runtime for production agents.

### 31.4 Potential pivot C: local context compiler

If portability works but decision cloning does not:

> Build the local minimal-context compiler for multi-model workflows.

All three pivots reuse the core architecture. The product should avoid a dead-end build.

---

## 32. Public Messaging

### 32.1 Homepage hero

```text
Your AI should know you.
You should own how.

SOUL is a local, verifiable model of your decisions and boundaries that works across every AI.
```

CTA:

```text
Build my SOUL
```

### 32.2 Product subhead

```text
Build your local SOUL in minutes and use the same verified identity inside the AI tools you already use. Improve it later with optional imports and corrections.
```

### 32.3 Developer message

```text
One context and authority layer for every model and agent.
```

### 32.4 Investor message

```text
Every person will use many agents. Those agents should not each invent a different owner. They should answer to one user-owned SOUL.
```

### 32.5 Trust message

```text
Local by default. Encrypted when synced. Inspectable before shared. Exportable forever.
```

---

## 33. Fundraising Deck Outline

### Slide 1: title

```text
SOUL
The user-owned identity and authority runtime for AI.
```

### Slide 2: problem

```text
Every AI builds a different model of the same person.
None is portable, measurable or authoritative.
```

### Slide 3: product

```text
Calibrate → Connect → Work → Improve → Govern
```

### Slide 4: demo

- rapid calibration;
- task written in an existing AI client;
- same SOUL connected to a second client;
- policy block.

### Slide 5: why current solutions fail

- vendor memory is locked in;
- portable profiles are passive;
- generic memory lacks proof;
- agents lack stable personal authority.

### Slide 6: wedge

```text
Take yourself with you when you switch AI.
```

### Slide 7: proof

- completed Blind Soul Tests;
- win-rate lift;
- activation time;
- second-model connections;
- early retention;
- token and latency metrics.

### Slide 8: business model

- free local core;
- Plus and Operator subscriptions;
- developer API;
- enterprise governance.

### Slide 9: market expansion

```text
personal context
→ decisions
→ agent authority
→ developer platform
→ organizational control plane
```

### Slide 10: moat

- personal evals;
- decision graph;
- policy history;
- context compiler;
- open format ecosystem;
- trust.

### Slide 11: competition

Position against:

- vendor-native memory;
- AI Passport products;
- memory infrastructure;
- agent governance tools.

### Slide 12: team and speed

- founder story;
- 14-day build;
- technical insight;
- product taste;
- user obsession.

### Slide 13: ask

- amount;
- runway;
- hiring plan;
- 12-month milestones.

---

## 34. Twelve-Month Plan

### Months 1-2

- fundraising MVP;
- 100 design users;
- benchmark methodology;
- core local runtime;
- first paid founder plan.

### Months 3-4

- encrypted sync;
- improved imports;
- two reliable agent integrations;
- Operator plan;
- first developer SDK users.

### Months 5-6

- policy replay;
- post-action verification;
- public red-team suite;
- 1,000 weekly active users;
- first team customers.

### Months 7-9

- policy packs;
- adapter SDK;
- organization support;
- model-provider partnerships;
- independent security assessment.

### Months 10-12

- hosted developer platform;
- enterprise controls;
- strong retention proof;
- repeatable B2B motion;
- open-format governance process;
- seed fundraising readiness.

### Twelve-month targets

These are ambition targets, not promises:

- 25,000 installs;
- 5,000 activated SOULs;
- 2,000 weekly active users;
- 500 paying consumers;
- 50 developer customers;
- 5 Team customers;
- 3 Business customers;
- 2 Enterprise customers;
- $250K-$350K ARR if the listed tier mix is achieved;
- 10 million governed decisions or context operations;
- published evidence of personalization lift;
- independent security review.

---

## 35. Team Plan

### Founder responsibilities

- product vision;
- desktop and web implementation;
- public building;
- user interviews;
- fundraising;
- demo and design;
- initial model/eval work.

### First technical hire

Profile:

- Rust and systems security;
- cryptography awareness;
- SQLite and sync;
- desktop software;
- agent protocol experience.

### First product/ML hire

Profile:

- evaluation science;
- retrieval and ranking;
- personalization;
- statistical methodology;
- model routing and cost optimization.

### Advisors

Useful early advisors:

- privacy/security engineer;
- consumer AI founder;
- applied statistician;
- identity or authorization specialist;
- youth-founder mentor for contracts and fundraising.

The product must not hire a large team before the blind-test wedge and retention are validated.

---

## 36. Build Quality Checklist

### Product

- onboarding under five minutes;
- one obvious hero action;
- no empty graph screen;
- no required ontology configuration;
- every inference shows evidence;
- every important change supports undo;
- every external disclosure has preview;
- every score explains methodology.

### Performance

- deterministic policy p95 under 5 ms;
- local hot path p95 under 75 ms;
- default context under 900 tokens;
- no UI blocking during ingestion;
- no duplicate extraction for unchanged chunks;
- no heavy reasoner on standard paths.

### Accuracy

- same-model blind comparisons;
- `Neither` option;
- held-out tests;
- strong baseline;
- contradiction handling;
- calibrated confidence;
- abstention supported;
- regression suite before release.

### Security

- encrypted local database;
- no plaintext cloud sync;
- no executable `.soul` imports;
- signed events;
- least-privilege MCP scopes;
- high-risk confirmation;
- secrets redaction;
- access receipts;
- full delete and export.

### Business

- free local ownership;
- no lifetime sync plan;
- no unlimited heavy inference;
- annual billing option;
- visible unit economics;
- paid intent measured early;
- no custom enterprise fork.

---

## 37. Final Product Definition

SOUL is successful when all of the following are true:

1. A user can create a useful personal model in minutes.
2. The model is stored locally and remains portable.
3. Personalization beats a strong simple baseline in blind tests.
4. The system knows when it lacks enough evidence.
5. Context disclosure is minimal and purpose-bound.
6. Most policy decisions cost zero tokens and complete locally.
7. Heavy reasoning is rare, bounded and explainable.
8. High-impact agent actions remain under human authority.
9. Every important memory, decision and policy has provenance.
10. Switching the intelligence provider does not erase the user.

The strongest final formulation is:

> **SOUL is a user-owned, verifiable runtime of human intent. It learns from how you decide, measures personalization through blind tests, carries context across models and enforces your boundaries on agent actions routed through its Gateway.**

The long-term vision is:

> Every person will use many models and hundreds of agents. Intelligence will be abundant and replaceable. The scarce layer will be a trusted, portable and executable definition of the human principal. SOUL intends to become that layer.

---

## 38. Source Snapshot

These links establish the July 29, 2026 market and technical context. Product strategy should be re-checked before fundraising because pricing, standards and competitors change quickly.

- Egoist Machines, YC Summer 2026: `https://www.ycombinator.com/companies/egoist-machines`
- Mem0 pricing: `https://mem0.ai/pricing`
- OpenAI Memory FAQ: `https://help.openai.com/en/articles/8590148-memory-faq`
- MCP authorization specification: `https://modelcontextprotocol.io/specification/2025-11-25/basic/authorization`
- Supabase RLS documentation: `https://supabase.com/docs/guides/database/postgres/row-level-security`
- Supabase database testing: `https://supabase.com/docs/guides/local-development/testing/overview`
- Tauri SQL plugin: `https://v2.tauri.app/plugin/sql/`
- Cloudflare AI Gateway: `https://developers.cloudflare.com/ai-gateway/`

---

## 39. Immediate Next Actions

1. Freeze the five MVP entity schemas.
2. Define the blind-test JSON format and statistical report.
3. Create the local SQLite event store and Tauri control center.
4. Build the five-minute guided calibration and activation flow.
5. Implement candidate review.
6. Implement the 900-token context compiler.
7. Add the local MCP runtime and secure Browser Companion bridge.
8. Connect coding clients plus ChatGPT Web and Gemini Web without adding a SOUL chat window.
9. Build randomized blind A/B rounds.
10. Add one deterministic spending boundary.
11. Record the 45-second demo before expanding scope.
12. Test with 20 external users and measure paid intent.

The order matters. Do not build the full agent operating system before proving that a verified personal model is meaningfully better than a short profile prompt.
