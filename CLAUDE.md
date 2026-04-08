# CLAUDE.md

## Development Rules

### Context First
- Always read `docs/` directory first to understand the current specification and project context before starting any work
- This ensures all changes are aligned with the documented design

### Specification Changes
- When any specification change occurs, always update `docs/SPECIFICATION.md` first
- Confirm the updated specification with the user before proceeding with implementation
- Do not implement changes that deviate from the documented specification without approval

### Development Methodology
- Follow TDD (Test-Driven Development) strictly
  1. Write failing tests first (Red)
  2. Implement minimum code to pass tests (Green)
  3. Refactor while keeping tests green (Refactor)
- Never skip writing tests before implementation

### Architecture Decision Records (ADR)
- Record significant design and architecture decisions as ADRs in `docs/adr/`
- Use the format: `docs/adr/NNNN-short-title.md` (e.g., `docs/adr/0001-use-ratatui-for-tui.md`)
- Each ADR should include: Title, Status (Proposed/Accepted/Deprecated/Superseded), Context, Decision, and Consequences
- Create an ADR when making decisions about: technology choices, architectural patterns, significant API design, data format choices, or any decision that future developers would benefit from understanding

### Code Quality
- All compiler warnings and clippy warnings must be resolved
- Run `cargo clippy` and `cargo fmt --check` before considering any change complete
- Do not leave warnings for later — fix them immediately

### Documentation
- All documentation must be written in English
- This includes README.md, docs/, ADRs, comments in code, and any other project documentation
