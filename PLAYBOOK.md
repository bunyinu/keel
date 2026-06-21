# Cash This Month — AI + Cold Outreach Playbook

**Your stack:** AI does delivery · You do outreach + calls · Target: first $2k–8k before month end

---

## The offer (one sentence)

> I set up Codex or Claude Code for your engineering team in 3 days — safe defaults, GitHub wired, one custom workflow — **$2,500 fixed** (startups) or **$4,500** (20–200 eng).

Buyers already have budget for Copilot/agents. You're not selling AI — you're selling **"it works Monday morning."**

---

## Who to message (50/day target)

| Title | Company size | Why they buy |
|-------|--------------|--------------|
| VP Engineering, Eng Director | 30–300 employees | Rolling out agents, scared of mess |
| Head of Platform / DevEx | 50–500 | Owns developer tooling |
| CTO (startups) | 15–80 | Decides fast, no procurement |
| Security / GRC (secondary) | 100+ | Cares about audit trail |

**Skip:** Fortune 500 (slow), solo devs (no budget), agencies (low margin).

**Find leads:** LinkedIn search, Wellfound, YC company list, "we use Claude" / "AI coding" posts on X, GitHub orgs hiring platform engineers.

---

## Cold DM / email (copy-paste)

### Version A — short (LinkedIn)

```
Hi {{name}} — quick one.

Teams rolling out Codex or Claude Code usually hit the same wall: no guardrails, messy PRs, nobody knows what the agent actually ran.

I do a 3-day setup: safe command policy, GitHub integration, audit log, one workflow tuned to your stack ({{their stack if known}}).

Fixed $2.5k. Two slots left this month.

Worth a 15-min call this week?
```

### Version B — email

**Subject:** Codex/Claude setup for {{company}} eng team

```
Hi {{name}},

Saw {{company}} is {{hiring eng / shipping fast / mentioned AI tools — personalize one line}}.

I help teams get Codex or Claude Code production-ready in 3 days:
• Approval rules (what agents can/can't touch)
• GitHub PR workflow + review checklist
• Basic audit log of agent actions
• One custom automation (tests on agent PRs, deploy preview, or internal API MCP)

Flat $2,500. No retainer.

If you're evaluating or already rolled out agents, I can show a 10-min before/after on a call {{day}} or {{day}}.

— {{your name}}
```

### Follow-up (day 3, day 7)

```
Day 3: Bumping this — happy to send a one-page scope doc if easier than a call.

Day 7: Last note from me. If agents aren't on your roadmap, ignore. If they are and setup is the blocker, I'm around.
```

---

## Discovery call (15 min — you + AI prep)

**Ask:**
1. Codex, Claude Code, or both? Already deployed or evaluating?
2. GitHub or GitLab? Monorepo?
3. Biggest fear — security, bad merges, cost, adoption?
4. One workflow they'd love automated this week?

**Close:**
> "I can start {{Monday}}. $2.5k, 50% upfront, rest on delivery. I'll send a one-pager tonight."

Use AI to generate the one-pager from call notes before you sleep.

---

## What you deliver (3 days — AI does 80%)

### Day 1 — Audit + config
- [ ] `AGENT_POLICY.md` — what agents may/may not do
- [ ] Hooks / approval config for their tool (see `deliverables/` templates)
- [ ] `.github/workflows/agent-pr-check.yml` if GitHub

### Day 2 — Integration
- [ ] One MCP or skill for their stack (Jira ticket → agent context, or staging deploy check)
- [ ] Team doc: "How we use agents here" (5 pages max)

### Day 3 — Handoff
- [ ] 30-min walkthrough (record Loom)
- [ ] `AUDIT_LOG.md` template + where logs live
- [ ] Invoice for remainder

**Package everything in a zip or private repo.** Looks professional. Repeatable.

---

## Pricing (don't negotiate below floor)

| Package | Price | Upfront |
|---------|-------|---------|
| Startup Setup | $2,500 | $1,250 |
| Team Setup (20+ eng) | $4,500 | $2,250 |
| Rush (48h) | +$1,000 | — |
| Add-on: second workflow | +$800 | — |

**Goal this month:** 1–2 closes = $2.5k–9k.

---

## Daily numbers (non-negotiable)

| Day | Outreach | Calls | Deliver |
|-----|----------|-------|---------|
| Mon–Fri | 50 messages | 2–3 booked | 1 client if signed |
| Weekend | 20 messages | prep / delivery | — |

**50 × 5 = 250 touches/week.** At 2% reply, 5 conversations. At 20% close, 1 deal/week. Math works if you don't stop.

---

## Objections

| They say | You say |
|----------|---------|
| "We do it in-house" | "Most teams do — I handle the boring policy + GitHub wiring so your seniors don't lose a sprint." |
| "Too expensive" | "One bad agent merge to prod costs more. Fixed scope, 3 days." |
| "Not on our roadmap" | "When it is, keep my email. Setup is the part everyone underestimates." |
| "Send case study" | "Early clients — I can show the deliverable pack anonymized on a call." (Use your templates.) |

---

## After first $2.5k

1. Ask for LinkedIn recommendation + intro to one peer CTO.
2. Productize: "Agent PR Gate" SaaS at $49/mo (reuse the GitHub Action).
3. Raise price to $3.5k — you have a reference.

---

## Your assets in this repo

- `deliverables/AGENT_POLICY.md` — client-ready policy template
- `deliverables/TEAM_PLAYBOOK.md` — how-to for their eng team
- `deliverables/github-workflows/agent-pr-check.yml` — GitHub Action you install for them
- `deliverables/OUTREACH_LIST.csv` — track leads
- `deliverables/SCOPE_ONE_PAGER.md` — send after discovery call

Start outreach today. First message in 10 minutes.
