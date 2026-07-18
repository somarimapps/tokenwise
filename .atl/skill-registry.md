# Skill Registry — tokenwise

Generated: 2026-07-17
Project: tokenwise
Stack: Rust / Cargo workspace

---

## User-Level Skills (`~/.claude/skills/`)

| Name | Trigger / Description | Path |
|------|----------------------|------|
| auditar-software | `/auditar-software`, adversarial software audit, technical premortem, find bugs, red team code | `~/.claude/skills/auditar-software/SKILL.md` |
| branch-pr | Creating, opening, or preparing PRs for review | `~/.claude/skills/branch-pr/SKILL.md` |
| chained-pr | PRs over 400 lines, stacked PRs, review slices, split oversized changes | `~/.claude/skills/chained-pr/SKILL.md` |
| codebase-memory | Explore codebase, understand architecture, trace call chains, graph queries, dependency analysis | `~/.claude/skills/codebase-memory/SKILL.md` |
| cognitive-doc-design | Writing guides, READMEs, RFCs, onboarding, architecture, or review-facing docs | `~/.claude/skills/cognitive-doc-design/SKILL.md` |
| comment-writer | PR feedback, issue replies, reviews, Slack messages, GitHub comments | `~/.claude/skills/comment-writer/SKILL.md` |
| compact-md | Convert binary files to compressed markdown via MarkItDown + Headroom | `~/.claude/skills/compact-md/SKILL.md` |
| defuddle | Extract clean markdown from web pages, online docs, articles, blog posts | `~/.claude/skills/defuddle/SKILL.md` |
| go-testing | Go tests, go test coverage, Bubbletea teatest, golden files | `~/.claude/skills/go-testing/SKILL.md` |
| graphify | `/graphify`, knowledge graph from codebase, docs, papers, images, architecture queries | `~/.claude/skills/graphify/SKILL.md` |
| issue-creation | Creating GitHub issues, bug reports, feature requests | `~/.claude/skills/issue-creation/SKILL.md` |
| json-canvas | Working with .canvas files, visual canvases, mind maps, flowcharts | `~/.claude/skills/json-canvas/SKILL.md` |
| judgment-day | `/judgment day`, dual review, adversarial review, blind judge, confirm before fixing | `~/.claude/skills/judgment-day/SKILL.md` |
| karpathy-mindset | Think Before Coding, Simplicity First, Surgical Changes, Goal-Driven Execution — any non-trivial coding task | `~/.claude/skills/karpathy-mindset/SKILL.md` |
| obsidian-bases | Working with .base files, Obsidian database views, table/card views, filters, formulas | `~/.claude/skills/obsidian-bases/SKILL.md` |
| obsidian-cli | Interact with Obsidian vault, manage notes, search vault, plugin/theme development | `~/.claude/skills/obsidian-cli/SKILL.md` |
| obsidian-markdown | Obsidian Flavored Markdown, wikilinks, callouts, frontmatter, embeds, .md files in Obsidian | `~/.claude/skills/obsidian-markdown/SKILL.md` |
| opt | `/opt`, `/opt normal`, `/opt full`, `/opt ultra`, response verbosity mode | `~/.claude/skills/opt/SKILL.md` |
| ponytail | Lazy senior dev mode, YAGNI, stdlib first, no unrequested abstractions | `~/.claude/skills/ponytail/SKILL.md` |
| ponytail-audit | Audit repo for over-engineering, delete/simplify/replace with stdlib | `~/.claude/skills/ponytail-audit/SKILL.md` |
| ponytail-debt | Harvest ponytail shortcut comments into debt ledger | `~/.claude/skills/ponytail-debt/SKILL.md` |
| ponytail-gain | Show ponytail measured impact scoreboard | `~/.claude/skills/ponytail-gain/SKILL.md` |
| ponytail-help | Quick reference for ponytail modes, skills, commands | `~/.claude/skills/ponytail-help/SKILL.md` |
| ponytail-review | Review diff for over-engineering, reinvented stdlib, needless deps | `~/.claude/skills/ponytail-review/SKILL.md` |
| skill-creator | New skills, agent instructions, documenting AI usage patterns | `~/.claude/skills/skill-creator/SKILL.md` |
| skill-improver | Improve skills, audit skills, refactor skills, skill quality | `~/.claude/skills/skill-improver/SKILL.md` |
| skill-registry | Update skills, skill registry, index available skills | `~/.claude/skills/skill-registry/SKILL.md` |
| somarim-tokens-reduction | Somarim/Customext projects, Angular/TypeScript, Odoo ERP, n8n/Shopify automations | `~/.claude/skills/somarim-tokens-reduction/SKILL.md` |
| work-unit-commits | Implementation, commit splitting, chained PRs, reviewable work units | `~/.claude/skills/work-unit-commits/SKILL.md` |

---

## Project-Level Skills

None detected. Project is new (Cargo workspace not yet scaffolded).
Add project-specific skills to `/Volumes/SSD-ATTECH/Proyectos/SOMARIMAPPS/TOKENWISE/.atl/skills/` when created.

---

## Excluded

- All `sdd-*` skills (orchestrator-managed, not registered here)
- All `_shared` utilities (not skills)

---

## Notes

- **Rust-relevant at apply time**: `karpathy-mindset`, `ponytail`, `work-unit-commits`, `branch-pr`, `chained-pr`, `judgment-day`
- **Doc-relevant**: `cognitive-doc-design`, `comment-writer`, `issue-creation`
- **Cross-cutting**: `compact-md`, `defuddle`, `graphify`, `auditar-software`
- `go-testing` is registered but not applicable to this Rust project (no Go code)
