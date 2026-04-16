# Description

This is a Rust demo project which represents a CLI tool that emulates a simple transaction server.

# Development rules
- For code simplicity avoid Rust lifetimes usage where possible. We are fine with the associated memory overhead given it makes the code easier to maintain.
- Put all deeply::nested::imports under the `use` statements, do not have them inline in the code.
- Do not add doc comments for trivial code lines.
- Avoid one character variable names.
- Usage of `unwrap()` or `expect()` outside of the tests code is forbidden. A proper value matching should be performed instead. 
- Once you are done with the code changes run the following tools to make sure the code is up to the coding standards:
  - `cargo fmt`
  - `cargo clippy`
- If any clippy issues appear, fix them.
- Do not commit any code unless explicitly asked by the user.


# Testing rules

- When writing tests do your best to avoid multiple assertions in one test. Break down the tests to atomic ones checking one thing per test.