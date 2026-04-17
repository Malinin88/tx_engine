# tx_engine

A streaming, single-threaded transaction processing engine. Reads a CSV of
transactions from a file path, processes deposits, withdrawals, disputes,
resolutions, and chargebacks against per-client accounts, and writes the
final account state as CSV to stdout.

## Build and run

```sh
cargo build --release
cargo run -- transactions.csv > accounts.csv
cargo test
```

Exactly one positional argument — the path to the input CSV. Anything else
prints a usage message to stderr and exits `1`.

## Input / output formats

**Input** (header required): `type, client, tx, amount`. Whitespace around
columns is tolerated. Amounts have up to four decimal places. `amount` is
required for `deposit`/`withdrawal` and must be absent for `dispute`,
`resolve`, `chargeback`.

**Output** (written to stdout): `client,available,held,total,locked`. One
row per client the engine has ever seen. Row order is unspecified. `total`
is computed as `available + held` at serialization time. Decimals are
rendered with their natural precision (`1.5`, not `1.5000`).

## Terminology

We use the term `Client` where banks would probably prefer `Account`. We do that since the requirements explicitly operate with the `Client` term and state the one client has 1 account.

## Documented assumptions

These resolve ambiguities in the spec:

1. **Locked is terminal.** Once locked, every subsequent transaction — of
   any kind — is silently rejected (the client is "frozen").
2. **Cross-client dispute references are ignored.** A dispute/resolve/
   chargeback whose `client` does not match the referenced tx's owning
   client is treated as "tx doesn't exist" and silently dropped.
3. **Re-dispute after resolve is allowed.** A resolved tx returns to
   `Normal` and may be disputed again. The spec does not forbid it.
4. **Dispute on a withdrawal applies the spec wording literally.**
   `available -= amount`, `held += amount`, which can drive `available`
   negative. We do not reinterpret semantics based on the original
   transaction type.
5. **Duplicate tx ids → stderr warning, row skipped.** The spec asserts
   tx ids are globally unique; a duplicate is malformed partner input.
6. **Negative amounts → stderr warning, row skipped.** A negative deposit
   would silently invert into a withdrawal. Zero is accepted (no-op on
   balance, tx id is recorded).

## Error handling

Three buckets:

| Bucket | Examples | Behavior | Exit |
|---|---|---|---|
| Fatal I/O | missing file, wrong header, write failure | stderr, exit `1`, no partial CSV | `1` |
| Malformed row | unknown kind, bad amount, negative amount, duplicate tx id | stderr warning, skip, continue | `0` |
| Business-rule failure | insufficient funds, dispute on missing/wrong-state tx, any tx on locked account | silent no-op, continue | `0` |

**Stdout is reserved for CSV output.** Every diagnostic goes to stderr so
redirection (`> accounts.csv`) and pipes stay clean.

## Possible concurrency model

The Engine is single-threaded by design, but its multiple instances can be used for sharded processing of the transactions list if the list is large:

- Shard clients across `N` workers by `client_id`.
- Each worker owns an `Engine` instance and processes its inbox serially.
- At shutdown, aggregate each worker's `snapshots()` into a merged list of records.

## Testing

Three tiers, all run by `cargo test`:

- **Unit tests in `client.rs`** — per-method state-machine coverage.
- **Unit tests in `io.rs` and `transaction.rs`** — CSV parse/write and
  header validation.
- **Integration tests in `tests/integration.rs`** — test the compiled
  binary against fixture CSV pairs under `tests/fixtures/`.

## AI usage

While creating this project AI tools (`Claude`, `Claude Code`) were used for:

- Domain-specific and general research
- Architecture brainstorming
- Testing strategy brainstorming
- Code generation

Where, for the sake of time saving, the code generation was done it was done under a strict developer's guidance by specifications provided.
Every generated bit of code was thoroughly reviewed and, where it was needed, manually improved. 
