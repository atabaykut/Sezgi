# Where Sezgi is going

[Türkçe](VIZYON.md)

**Goal:** a universal software intelligence that lets an AI read any software once, truly understand it, and work in it safely.

Today's Sezgi is the first step: it reads an application's code, builds a map, and gives the assistant short answers with sources and trust stamps. This page puts where we are going and where we are today side by side, honestly.

## Three principles

1. **It does not make things up.** Every fact comes with its source (file:line) and a trust stamp: certain > observed > inferred > unknown. Sezgi says "unknown" when it does not know.
2. **Nothing unsafe goes without approval.** Where Sezgi is not sure, the answer is approval (fail-closed). On real data every write needs the human's password.
3. **The assistant asks instead of scanning code.** Answers must be short: cheaper in tokens than answering the same question by scanning the code.

## The 17 founding questions

A connected assistant should answer these about an application it has never seen, within its first minute, correctly, with sources and honestly. The status column is the latest measurement on three held-out open-source apps (NestJS, Django, Next.js), 2026-11-03.

| # | Question | Today |
|---|---|---|
| 1 | What is the application? | Partly: from the README and domain entities; domain classification is sometimes wrong |
| 2 | Why does it exist? | Partly: quoted from the README |
| 3 | What business problem does it solve? | Partly: sometimes picks a setup sentence |
| 4 | Who are its users? | Partly: from role names |
| 5 | What can a user do? | Works: almost all operations are found; unreadable routes are reported |
| 6 | Which features actually work? | Partly: only with a network capture or live-mode observation |
| 7 | Which screen starts which operation? | Weak: known request patterns only |
| 8 | Which functions call each other? | Partly: call graph in eight languages |
| 9 | Which tables are affected? | Partly: ORMs and raw SQL, cascades included |
| 10 | What are the preconditions? | Partly: validation rules and rejections inside services |
| 11 | Who may do what? | Partly: the login decision is reliable in most frameworks; roles and custom auth decorators can be missed |
| 12 | Does it affect other systems? | Partly: HTTP, queues, e-mail, payment, cloud; "unknown" when the chain has an unclassified call |
| 13 | Can it be undone? | Partly: soft delete, inverse operations, revision history, permanent delete |
| 14 | Does it need human approval? | Partly: the decision is fail-closed; the stated reason is not always right |
| 15 | Is this certain, inferred or unknown? | Partly: every claim is stamped; wrong claims stamped "certain" can still occur |
| 16 | What becomes stale when code changes? | Works: staleness tied to content digests |
| 17 | Which operations does a task need? | Partly: Turkish and English task plans; unrecognised steps are left empty |

In the same measurement about half of the hand-checked claims were correct. This release's fixes (permission decorators, soft delete, external effects, reversibility) aim at that number. The target is **above 90% accuracy on never-seen apps and zero wrong claims stamped "certain"**.

## From a map to understanding

Today the map is centred on operations (endpoints). The goal is one meaning graph of the whole application:

- **Nodes:** screen, component, operation, function, table, business rule, user type, user journey, external system.
- **Edges:** calls, reads, writes, deletes, triggers, authorizes, undoes.
- **Behaviour summary (DNA):** for every operation who may do it, preconditions, failure modes, data effect, external effect and reversibility, all with evidence.

The graph and the evidence store exist today (`.sezgi/graf.json`, `.sezgi/kanit.json`). What is missing is depth in screen and business-rule nodes and the accuracy of every edge.

## Beyond source code

- **Systems without source:** .exe, .dll, APK, SQLite and desktop apps; with REA, native binaries, firmware and process observation. Today at identity and schema level.
- **Run-time observation:** network captures and browser scenarios separate "exists in code" from "actually works".
- **Many applications:** the switchboard joins several apps in one connection; entity mappings across apps ("this customer is that record over there") and approved change plans.

## Benlik Card

Every application introduces itself with a signed, standard card: what it does, who may do what, which operation needs approval. The specification is open ([benlik/SPEC.md](https://github.com/atabaykut/Sezgi/blob/main/benlik/SPEC.md)). 1.0 goal: two independent card generators and an agent that consumes the card.

## Next

1. **Accuracy:** the held-out measurement is repeated and published with every release; wrong "certain" claims go to zero.
2. **Coverage:** Supabase (edge functions and SQL functions), Spring servlet filters, Next.js `basePath`, server templates (Blade, ERB, Jinja).
3. **Assistant experience:** filtered listing ("writes that need no login"), callers/callees by short name, "what breaks if I change this file".
4. **1.0:** independent security audit, two card generators, a consuming agent.
