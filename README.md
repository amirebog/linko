# Linko

Linko is a local-first personal website builder. Register an account, design your public page with content blocks, publish it at `/{username}`, and manage everything from a private dashboard.

## Requirements

- Rust stable (install with [rustup](https://rustup.rs/))
- No separate database server

Windows: `winget install Rustlang.Rustup`, then open a new terminal.

## Run on Windows

From this directory:

```bat
run-windows.bat
```

Or:

```bat
cargo run
```

Default URL: `http://127.0.0.1:3000`

On first start the app creates `data/` if needed, opens `data/linko.db`, applies migrations, then binds the HTTP server.

## Configuration

Copy `.env.example` to `.env` (the Windows script does this automatically).

| Variable | Meaning |
| --- | --- |
| `LINKO_HOST` | Bind host (default `127.0.0.1`) |
| `LINKO_PORT` | Bind port (default `3000`) |
| `LINKO_DATABASE_PATH` | SQLite path (default `data/linko.db`) |
| `LINKO_SESSION_HOURS` | Session lifetime |
| `LINKO_SECURE_COOKIES` | Set `true` behind HTTPS |
| `LINKO_BOOTSTRAP_ADMIN` | Username to promote when no admin exists |
| `RUST_LOG` | Tracing filter |

`.env` is loaded from the current working directory and from the crate directory.

## First administrator

New registrations always receive the `user` role. Admin is never chosen on the public form.

1. Register normally in the UI.
2. Stop the server.
3. Set `LINKO_BOOTSTRAP_ADMIN=your_username` in `.env`.
4. Start again. If no admin exists, that user is promoted once.

Or:

```bat
cargo run -- bootstrap-admin your_username
```

The last active administrator cannot be disabled or deleted.

## Local database

Default file: `data/linko.db` relative to the process working directory.

SQLite uses WAL, foreign keys, a busy timeout, and connection pooling. The database is never wiped on startup.

## Tests

```bat
cargo test
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

Tests use temporary SQLite files and do not touch the development database.

## Security notes

Implemented for this local MVP:

- Argon2id password hashing
- Hashed session tokens, HttpOnly cookies, SameSite=Lax
- CSRF double-submit for cookie-authenticated mutations
- Origin checks when the Origin header is present
- Login rate limiting (per IP, in-process)
- Ownership checks on every page/block mutation
- Admin authorization on the server
- Output escaping for user content
- External URL allow-list (`http`/`https`)
- Security headers and a 1 MiB body limit

Limitations: this is a single-process local app. In-memory rate limits reset on restart. There is no reverse proxy, TLS terminator, or multi-node session store. Do not expose it to the public internet without HTTPS, a hardened host, and an operational review.
