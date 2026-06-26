# GIR

`GIR` is a project that helps for generating safe Rust bindings for GObject based libraries. The generated bindings consists of two parts: FFI (the unsafe 1:1 C API calls from Rust) and the high-level, safe Rust API.

## How to use

A work in progress book to help with learning how to use `gir` along with a tutorial are available at <https://gtk-rs.org/gir/book>.

If you intend to contribute to `gir` or make use of `libgir`, the docs are available at <https://gtk-rs.org/gir/docs/gir> / <https://gtk-rs.org/gir/docs/libgir>.

## AI Contribution Policy

gtk-rs is a project by humans for humans. We prefer contributions that
are produced by human creativity, we expect a human to take full
responsibility for each contribution, and we will take more joy in
reviewing contributions when there's people at the other end of the
line to stand by their changes.

If you use LLM/GenAI tools for your contributions, here are the rules
you must follow:

### Requirements

1. Use AI as a tool. Verify behavior, correctness, and compatibility
   yourself prior to submitting your contribution. Do not ask the
   maintainers to do this for you.
1. Keep changes narrow and limited. Do **NOT** use LLM/GenAI tools to
   generate broad rewrites, large refactorings, or style changes.
1. Do **NOT** submit generated code, documentation, or tests that you
   don’t understand.
1. Do **NOT** fabricate benchmarks, bug reports, test results, code
   samples, or reproducers.
1. Do **NOT** include private code, credentials, tokens, or any other
   confidential material.
1. Respect the licensing and attribution requirements.

### Disclosure

Always disclose the use of LLM/GenAI tools when creating an issue or
a merge request. Do not include trailers like “Co-authored-by:” or
“Assisted-by:” in commit messages, since they serve as free advertising
for AI companies.

### Reviews

1. Describe your changes, and the verification steps.
1. Be prepared to explain all the changes yourself.
1. Do **NOT** feed the review feedback to an LLM/GenAI tool.

### Maintainers expectations

1. Review LLM/GenAI-assisted contributions more strictly than any other contribution.
1. Require reproducibility in fixes and tests.
1. Reject changes that appear to be unverified LLM/GenAI output.
1. Reject comments and feedback that appear to be LLM/GenAI output.

> A COMPUTER CAN NEVER BE HELD ACCOUNTABLE.
> THEREFORE A COMPUTER MUST NEVER MAKE A MAINTENANCE DECISION.

