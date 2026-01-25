# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.0.0] - 2026-01-25

### Production-Ready CLI

**Initial release with comprehensive error handling, security hardening, code quality improvements, test coverage, developer experience enhancements, and LLM-optimized performance.**

### Added

- **Error handling framework**
  - LLM-E### error codes with severity and remediation hints
  - Structured JSON responses with error chains
  - Dual-mode output (human-readable or JSON)

- **Security hardening**
  - ReDoS prevention via 10KB regex size limit
  - Resource bounds validation on all parameters
  - Path traversal blocking with canonicalize()

- **Code quality refactoring**
  - SearchOptions struct consolidates 11-16 parameters
  - Shared output formatting module eliminates duplication
  - 90% parameter reduction across search functions

- **Comprehensive test coverage**
  - 118 unit tests with 87.72% coverage
  - Tests for scoring algorithm, SQL query builders, CLI parsing
  - Public API tests for all search functions

- **Developer experience**
  - Clear error messages with actionable remediation
  - Database existence validation before connection
  - File read error logging with context
  - Distinct "not found" vs "corrupted" error messages

- **Performance optimization**
  - `--sort-by {relevance|position}` flag for LLM-optimized or performance modes
  - Position mode skips in-memory scoring for pure SQL ORDER BY
  - Relevance mode uses intelligent ranking (exact > prefix > contains)

- **CLI documentation**
  - 8 usage examples in help text
  - Covers basic search, regex, JSON output, path filtering, all modes
  - Documents `--db`, `--output`, filtering flags, and `--sort-by`

### Changed

- Consolidated 11-16 parameters into single SearchOptions struct
- Replaced three near-identical output functions with shared formatting
- Fixed ReDoS vulnerability in regex compilation
- Added path validation blocking for sensitive directories

### Security

- All regex input validated for size limits (prevents catastrophic backtracking)
- Resource parameters enforce upper bounds at parse time
- Path validation blocks traversal attempts and symlinks to sensitive dirs

### Tested

- 118 unit tests passing
- 87.72% code coverage on src/query.rs
- Zero clippy warnings
- Release build verified

[Unreleased]: Initial llmgrep CLI with Magellan DB search
