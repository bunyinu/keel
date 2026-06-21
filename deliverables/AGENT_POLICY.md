# Agent Usage Policy — {{COMPANY_NAME}}

**Effective:** {{DATE}}  
**Owner:** {{ENG_LEAD}}  
**Tools covered:** Codex · Claude Code

---

## Purpose

This policy defines how {{COMPANY_NAME}} uses AI coding agents safely: what they may access, what requires human approval, and how we audit agent activity.

---

## Approved use

Agents **may** be used for:

- Feature implementation on non-production branches
- Writing and updating tests
- Refactoring with existing test coverage
- Documentation and README updates
- Explaining legacy code
- Drafting PR descriptions and review responses

---

## Requires human review before merge

All agent-generated code **must**:

1. Pass CI (tests, lint, typecheck)
2. Be reviewed by a human engineer (not another agent)
3. Have a PR description noting agent assistance

---

## Prohibited without explicit approval

Agents **must not** (without written approval from {{ENG_LEAD}} or Security):

| Action | Reason |
|--------|--------|
| Modify production infrastructure (Terraform apply, kubectl to prod) | Blast radius |
| Access or commit secrets (`.env`, vault files, API keys) | Security |
| Disable tests, linters, or security scanners | Trust erosion |
| Push directly to `main`, `master`, or release branches | Change control |
| Install new dependencies without PR review | Supply chain |
| Exfiltrate customer data or run queries against prod DBs | Data protection |

---

## Environment boundaries

| Environment | Agent access |
|-------------|--------------|
| Local dev | Allowed with policy hooks enabled |
| CI / preview | Allowed via GitHub Actions only |
| Staging | Read-only unless ticket explicitly allows |
| Production | **No direct agent access** |

---

## Secrets handling

- Never paste secrets into agent prompts
- Use environment variables and secret managers only
- Rotate any credential accidentally exposed in a session

---

## Audit

- Agent sessions should be logged where the tool supports it
- GitHub PRs must tag `agent-assisted` label when applicable
- Monthly spot-check: 5 random agent PRs reviewed for policy compliance

---

## Incident response

If an agent causes a production issue or exposes secrets:

1. Revert the change immediately
2. Notify {{ENG_LEAD}} and Security within 1 hour
3. Document in postmortem template
4. Tighten hooks/policy before resuming agent use on affected repos

---

## Acknowledgment

Engineers using agents at {{COMPANY_NAME}} are expected to follow this policy. Questions: {{ENG_LEAD_EMAIL}}
