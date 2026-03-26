# Development Lifecycle — HeadlessCraft

Every change follows a structured lifecycle:

```
Identify → Research → Arch Review Gate → ADR → Plan → Test First → Implement → Review → Integrate → Retrospect
```

## Stages

### 1. Identify
Recognize the need — bug report, feature request, phase task, or improvement.

### 2. Research
Understand the problem space. Read the vanilla Java reference, existing code, and relevant ADRs.

### 3. Arch Review Gate (Stage 2.5)
Before planning, question every constraining ADR:
- Is this still the right pattern?
- Would a Rust developer choose this?
- Does it make sense for a client library?
- Will we regret this in 6 months?

If an ADR is outdated → create a superseding ADR first.

### 4. ADR
Record significant decisions as Architecture Decision Records in `docs/adr/`.

### 5. Plan
Create a structured plan with tasks, dependencies, and acceptance criteria.

### 6. Test First (TDD)
Write failing tests before implementation. Tests define the expected behavior.

### 7. Implement
Write the minimum code to make tests pass. Then refactor.

### 8. Review
Code review against ADRs, correctness, testing, and vanilla compliance.

### 9. Integrate
Merge to main. CI must pass. Never leave main broken.

### 10. Retrospect
After phases: update memories.md, review ADRs, record learnings.

## Lifecycle Variants

**Trivial changes** (typo fixes, dep bumps): abbreviated lifecycle — fix, test, commit.
**Emergency fixes**: hotfix branch, minimal review, retrospect after.
