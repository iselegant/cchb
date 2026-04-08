# CLAUDE.md

## Development Rules

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

### Documentation
- All documentation must be written in English
- This includes README.md, docs/, comments in code, and any other project documentation
