# FlashLAN Repository Instructions

## UI work

These instructions apply to every change under `src/layouts`, `src/views`, `src/ui`, and `src/styles`.

Before editing user-visible UI:

1. Read `/DESIGN.md` completely.
2. Read `/docs/design/page-contracts.md` for the affected screen or Pattern.
3. Use `/docs/design/visual-checklist.md` before declaring the work complete.

Treat every `MUST` and `MUST NOT` rule in `DESIGN.md` as a repository requirement.

Additional implementation rules:

- Reuse `src/ui/components` and project Patterns instead of recreating primitives in a view.
- Do not add arbitrary UnoCSS visual values such as `rounded-[...]`, `text-[...]`, custom shadows, custom spacing, or direct color values in views and layouts.
- Use semantic theme classes such as `primary`, `muted`, `border`, `success`, and `destructive`; do not use direct palette colors for product meaning.
- Prefer compact list rows over repeated Cards for devices, transfers, conversations, and settings.
- Do not change transfer, discovery, trust, history, or messaging behavior during a design-only task.
- Existing violations may be migrated incrementally, but touched UI must not introduce new violations.
- When a requested design intentionally conflicts with the contract, follow the explicit user request and update the contract or document the scoped exception.

For UI changes, verify the relevant desktop and mobile states and run `pnpm check` before completion.
