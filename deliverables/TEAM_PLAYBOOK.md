# How We Use AI Coding Agents — {{COMPANY_NAME}}

Quick reference for engineers. Read this before your first agent session.

---

## Tools

| Tool | Use for | Install |
|------|---------|---------|
| **Claude Code** | Multi-file features, architecture, complex refactors | `claude` in repo root |
| **Codex** | Parallel tasks, terminal workflows, reviews | `codex` in repo root |

Pick one per task. Don't run both on the same branch without coordinating.

---

## Standard workflow

```
1. Create branch:  agent/{{ticket-id}}-short-description
2. Write a clear prompt (see template below)
3. Let agent work — approve commands carefully
4. Run tests locally:  npm test / make test / etc.
5. Open PR with label: agent-assisted
6. Human review + CI must pass
7. Merge
```

---

## Prompt template

```markdown
## Task
[One sentence goal]

## Context
- Ticket: JIRA-123
- Branch: agent/JIRA-123-user-settings
- Do NOT touch: auth/, payments/, migrations/

## Acceptance criteria
- [ ] ...
- [ ] Tests pass
- [ ] No new dependencies without asking

## Constraints
- Follow existing patterns in src/services/
- Max 5 files changed unless you explain why
```

---

## What to approve vs deny

| Approve | Deny (escalate to lead) |
|---------|-------------------------|
| `npm test`, `git status`, `git diff` | `rm -rf`, `curl | bash` |
| Reading files in repo | Writing to `.env` or `secrets/` |
| Installing deps in feature branch | `terraform apply`, prod kubectl |
| Creating commits on feature branch | Pushing to main |

---

## PR checklist (agent-assisted)

- [ ] Label: `agent-assisted`
- [ ] Description says what the agent did
- [ ] CI green
- [ ] You read every changed line
- [ ] No secrets in diff
- [ ] Screenshots/logs if UI or behavior changed

---

## Getting help

- Policy questions: {{ENG_LEAD}}
- Tool broken: #eng-agents Slack channel
- Security concern: security@{{COMPANY_DOMAIN}}

---

## Tips that actually work

1. **Small tasks win** — "Add validation to signup form" beats "rebuild auth"
2. **Point at examples** — "Match the pattern in `src/services/billing.ts`"
3. **Stop and reset** — if agent loops 3+ times, start fresh with narrower scope
4. **Review diffs like a junior's PR** — agents are fast, not infallible
