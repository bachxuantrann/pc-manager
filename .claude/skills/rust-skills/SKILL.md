---
name: rust-skills
description: >
  Rust coding guidelines for THIS project (PC Manager, Windows desktop app, Tauri 2, edition 2021).
  Trimmed from leonardomso/rust-skills: only rules that apply here and do not conflict with CLAUDE.md.
  Use when writing, reviewing or refactoring Rust code in src-tauri/.
license: MIT
metadata:
  author: leonardomso (trimmed for pc-manager — see SOURCE.txt)
  version: "1.5.1-pcm"
---

# Rust guidelines — PC Manager

129 rules. Each id maps to `rules/<id>.md` (Bad/Good examples) — open only the files you need.

**CLAUDE.md always wins.** Project facts that override generic advice below:
- No `unwrap()`/`expect()` outside tests; errors go to the UI as `AppError { code, message }`.
- `unsafe` only in `src-tauri/src/sys/`, each block with `// SAFETY:`.
- Edition 2021, MSRV 1.85, stable only. Release keeps `panic = "unwind"`; no `target-cpu=native`.
- No new crates without the user's consent (no tokio/tracing/smallvec/ahash/arena/test frameworks…).
- Locks via `crate::sync::LockExt` (`lock_recover`); background loops wrapped in `sync::survive`.
- `serde(deny_unknown_fields)` only for data embedded at build time (`cleaner.toml`), never for saved user data.
- Slice strings with `s.get(..n)`, search on `to_ascii_lowercase()` (keeps byte offsets).

## When to read which group

| Task | Groups |
|------|--------|
| New function / refactor | `own-`, `err-`, `name-`, `pat-` |
| New type / IPC payload | `type-`, `conv-`, `serde-` |
| Win32 / FFI (`sys/`) | `unsafe-`, `num-`, `type-` |
| Background work, scanning | `conc-`, `mem-`, `perf-` |
| Review | `anti-`, `lint-` |

### Ownership & Borrowing (`own-`)
- `own-borrow-over-clone` — Prefer `&T` borrowing over `.clone()`
- `own-slice-over-vec` — Accept `&[T]` not `&Vec<T>`, `&str` not `&String`
- `own-cow-conditional` — Use `Cow<'a, T>` for conditional ownership
- `own-arc-shared` — Use `Arc<T>` for thread-safe shared ownership
- `own-rc-single-thread` — Use `Rc<T>` for shared ownership in single-threaded contexts
- `own-refcell-interior` — Use `RefCell<T>` for interior mutability in single-threaded code
- `own-mutex-interior` — Use `Mutex<T>` for interior mutability across threads
- `own-rwlock-readers` — Use `RwLock<T>` when reads significantly outnumber writes
- `own-copy-small` — Implement `Copy` for small, simple types
- `own-clone-explicit` — Use explicit `Clone` for types where copying has meaningful cost
- `own-move-large` — Move large types instead of copying; use `Box` if moves are expensive
- `own-lifetime-elision` — Rely on lifetime elision rules; add explicit lifetimes only when required

### Error Handling (`err-`)
- `err-thiserror-lib` — Use `thiserror` for library error types
- `err-anyhow-app` — Use `anyhow` for application error handling
- `err-result-over-panic` — Return `Result<T, E>` instead of panicking for recoverable errors
- `err-context-chain` — Add context with `.context()` or `.with_context()`
- `err-no-unwrap-prod` — Avoid `unwrap()` in production code; use `?`, `expect()`, or handle errors
- `err-question-mark` — Use `?` operator for clean propagation
- `err-from-impl` — Implement `From<E>` for error conversions to enable `?` operator
- `err-source-chain` — Preserve error chains with `#[source]` or `source()` method
- `err-lowercase-msg` — Start error messages lowercase, no trailing punctuation
- `err-doc-errors` — Document error conditions with `# Errors` section in doc comments
- `err-custom-type` — Define custom error types for domain-specific failures

### Memory (`mem-`)
- `mem-with-capacity` — Use `with_capacity()` when size is known
- `mem-box-large-variant` — Box large enum variants to reduce overall enum size
- `mem-boxed-slice` — Use `Box<[T]>` instead of `Vec<T>` for fixed-size heap data
- `mem-clone-from` — Use `clone_from()` to reuse allocations when repeatedly cloning
- `mem-reuse-collections` — Clear and reuse collections instead of creating new ones in loops
- `mem-avoid-format` — Avoid `format!()` when string literals work
- `mem-write-over-format` — Use `write!()` into existing buffers instead of `format!()` allocations
- `mem-zero-copy` — Use zero-copy patterns with slices and `Bytes`
- `mem-smaller-integers` — Use appropriately-sized integers to reduce memory footprint
- `mem-take-replace` — Use `mem::take` / `mem::replace` to move a value out of a `&mut` without cloning
- `mem-drop-order` — Know and control drop order: struct fields drop top-to-bottom, locals in reverse

### Unsafe Code (`unsafe-`)
- `unsafe-safety-comment` — Write a `// SAFETY:` comment above every `unsafe` block and a `# Safety` section in every `unsafe fn`.
- `unsafe-minimize-scope` — Keep `unsafe` blocks as small as possible — mark only the operation that requires unsafety, not the surrounding safe code.
- `unsafe-maybeuninit` — Use `MaybeUninit<T>` for uninitialized memory; never use `mem::uninitialized()` or `mem::zeroed()` for types with validity invariants.
- `unsafe-extern-block` — In Rust 2024, wrap `extern` blocks in `unsafe extern { }` and annotate each item as `safe` or `unsafe`.
- `unsafe-send-sync-manual` — Document the invariants when manually implementing `Send` or `Sync`; prefer letting the compiler derive them automatically.
- `unsafe-no-mangle-unsafe` — In Rust 2024, write `#[unsafe(no_mangle)]`, `#[unsafe(export_name = "...")]`, and `#[unsafe(link_section = "...")]` — not the bare attribute forms.

### Concurrency (rayon, atomics, threads) (`conc-`)
- `conc-rayon-par-iter` — Use rayon's `par_iter()` for CPU-bound data parallelism
- `conc-scoped-threads` — Use `std::thread::scope` to borrow stack data across threads
- `conc-atomic-ordering` — Use the weakest correct memory `Ordering` for every atomic operation
- `conc-thread-local` — Prefer `thread_local!` with `Cell`/`RefCell` over `static mut`

### Numeric Safety (`num-`)
- `num-overflow-explicit` — Handle integer overflow explicitly: `checked_`/`saturating_`/`wrapping_`/`overflowing_`
- `num-cast-try-from` — Avoid `as` for narrowing casts; use `From` for widening and `TryFrom` for narrowing
- `num-float-compare` — Don't compare floats with `==`; use a tolerance, and `total_cmp` for ordering
- `num-saturating-clamp` — Bound values with `clamp` and saturating arithmetic
- `num-nonzero` — Use `NonZero*` types to forbid zero and unlock the niche optimization

### Type Safety (`type-`)
- `type-newtype-ids` — Wrap IDs in newtypes: `UserId(u64)`
- `type-newtype-validated` — Use newtypes to enforce validation at construction time
- `type-enum-states` — Use enums for mutually exclusive states
- `type-option-nullable` — Use `Option<T>` for values that might not exist
- `type-result-fallible` — Use `Result<T, E>` for operations that can fail
- `type-phantom-marker` — Use `PhantomData` to express type relationships without runtime cost
- `type-never-diverge` — Use `!` (never type) for functions that never return
- `type-generic-bounds` — Add trait bounds only where needed, prefer where clauses for readability
- `type-no-stringly` — Avoid stringly-typed APIs; use enums, newtypes, or validated types
- `type-repr-transparent` — Use `#[repr(transparent)]` for newtypes in FFI contexts
- `type-deref-coercion` — Implement `Deref`/`DerefMut` only for smart-pointer and transparent wrapper types
- `type-display-vs-debug` — Use `Display` for user-facing output and `Debug` for diagnostics; never swap them
- `type-numeric-fmt` — Implement `LowerHex`, `UpperHex`, `Octal`, and `Binary` for numeric newtypes

### Conversions (`conv-`)
- `conv-tryfrom-fallible` — Implement `TryFrom` for fallible conversions instead of ad-hoc conversion functions
- `conv-fromstr-parsing` — Implement `FromStr` to enable `str::parse` for string-to-type conversions
- `conv-asmut-mutable` — Accept `impl AsMut<T>` for flexible mutable borrowed inputs instead of concrete mutable references

### Serde (`serde-`)
- `serde-rename-all` — Match the external naming convention with `#[serde(rename_all = ...)]`
- `serde-default-compat` — Use `#[serde(default)]` for optional and backward-compatible fields
- `serde-skip-empty` — Omit empty fields with `skip_serializing_if`
- `serde-flatten` — Inline nested structs or capture extra keys with `#[serde(flatten)]`
- `serde-enum-representation` — Choose enum tagging deliberately: externally, internally, adjacently tagged, or untagged
- `serde-custom-with` — Customize a field's (de)serialization with `with` / `serialize_with` / `deserialize_with`
- `serde-try-from-validate` — Validate while deserializing with `#[serde(try_from = "Raw")]`

### Pattern Matching (`pat-`)
- `pat-let-else` — Use `let ... else` for early-return pattern extraction
- `pat-matches-macro` — Use `matches!()` for boolean pattern tests
- `pat-exhaustive-enum` — Match owned enums exhaustively; avoid catch-all `_` that hides new variants
- `pat-at-bindings` — Use `@` bindings to capture a value while matching it against a pattern

### Collections (`coll-`)
- `coll-binaryheap` — Use `BinaryHeap` for a priority queue or repeated max-extraction
- `coll-map-choice` — Pick the map by access pattern: `HashMap` (fast, unordered), `BTreeMap` (sorted / range queries), `IndexMap` (insertion order)
- `coll-seq-choice` — Default to `Vec`; use `VecDeque` for queue/deque behaviour; avoid `LinkedList`
- `coll-set-membership` — Use `HashSet`/`BTreeSet` for membership tests and dedup, not linear `Vec::contains`

### Naming (`name-`)
- `name-types-camel` — Use `UpperCamelCase` for types, traits, and enum names
- `name-variants-camel` — Use `UpperCamelCase` for enum variants
- `name-funcs-snake` — Use `snake_case` for functions, methods, variables, and modules
- `name-consts-screaming` — Use `SCREAMING_SNAKE_CASE` for constants and statics
- `name-lifetime-short` — Use short, conventional lifetime names: `'a`, `'b`, `'de`, `'src`
- `name-type-param-single` — Use single uppercase letters for type parameters: `T`, `E`, `K`, `V`
- `name-as-free` — `as_` prefix: free reference conversion
- `name-to-expensive` — Use `to_` prefix for expensive conversions that allocate or compute
- `name-into-ownership` — Use `into_` prefix for ownership-consuming conversions
- `name-no-get-prefix` — Omit get_ prefix for simple getters
- `name-is-has-bool` — Use `is_`, `has_`, `can_`, `should_` prefixes for boolean-returning methods
- `name-iter-convention` — Use iter/iter_mut/into_iter for iterator methods
- `name-iter-method` — Name iterator methods `iter()`, `iter_mut()`, and `into_iter()` consistently
- `name-iter-type-match` — Name iterator types after their source method
- `name-acronym-word` — Treat acronyms as words in identifiers: `HttpServer`, not `HTTPServer`

### Performance Patterns (`perf-`)
- `perf-iter-over-index` — Prefer iterators over manual indexing
- `perf-iter-lazy` — Keep iterators lazy, collect only when needed
- `perf-collect-once` — Don't collect intermediate iterators
- `perf-entry-api` — Use entry API for map insert-or-update
- `perf-drain-reuse` — Use drain to reuse allocations
- `perf-extend-batch` — Use extend for batch insertions
- `perf-chain-avoid` — Avoid chain in hot loops
- `perf-profile-first` — Profile before optimizing
- `perf-io-buffering` — Wrap `Read`/`Write` in `BufReader`/`BufWriter` for many small operations

### Clippy & Linting (`lint-`)
- `lint-deny-correctness` — `#![deny(clippy::correctness)]`
- `lint-warn-suspicious` — Enable clippy::suspicious for likely bugs
- `lint-warn-style` — Enable clippy::style for idiomatic code
- `lint-warn-complexity` — Enable clippy::complexity for simpler code
- `lint-warn-perf` — Enable clippy::perf for performance improvements
- `lint-pedantic-selective` — Enable clippy::pedantic selectively
- `lint-missing-docs` — Warn on missing documentation for public items
- `lint-unsafe-doc` — Require documentation for unsafe blocks
- `lint-rustfmt-check` — Run cargo fmt --check in CI
- `lint-cfg-check` — Enable `unexpected_cfgs` and declare known cfgs to catch feature-gate typos
- `lint-clippy-nursery-selected` — Enable high-value `clippy::nursery` lints selectively, not the whole group

### Anti-patterns (`anti-`)
- `anti-unwrap-abuse` — Don't use `.unwrap()` in production code
- `anti-expect-lazy` — Don't use expect for recoverable errors
- `anti-clone-excessive` — Don't clone when borrowing works
- `anti-string-for-str` — Don't accept &String when &str works
- `anti-vec-for-slice` — Don't accept &Vec<T> when &[T] works
- `anti-index-over-iter` — Don't use indexing when iterators work
- `anti-panic-expected` — Don't panic on expected or recoverable errors
- `anti-empty-catch` — Don't silently ignore errors
- `anti-over-abstraction` — Don't over-abstract with excessive generics
- `anti-premature-optimize` — Don't optimize before profiling
- `anti-type-erasure` — Don't use Box<dyn Trait> when impl Trait works
- `anti-format-hot-path` — Don't use format! in hot paths
- `anti-collect-intermediate` — Don't collect intermediate iterators
- `anti-stringly-typed` — Don't use strings where enums or newtypes would provide type safety

---
Source and what was cut: `SOURCE.txt`.
