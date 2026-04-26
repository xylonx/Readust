# readust

readust is a lightweight backend for [Readest](https://readest.com/) compatible clients. It replaces the original, larger backend stack with a small Rust service that provides Readest compatible APIs.

The service is built with Axum, PostgreSQL, SQLx, JWT authentication, and S3-compatible object storage. It is intended for private or self-hosted deployments where you want the Readest client experience without operating the full upstream backend.

## Features

- Readest-compatible auth endpoints under `/auth/v1`
- Email and password signup/login with bcrypt password hashing
- JWT access tokens and refresh-token rotation
- Optional signup lockout for private deployments
- Sync API for books, book configs, reading progress, and notes
- Storage API for listing files, upload/download presigned URLs, deletion, purge, and usage stats
- PostgreSQL persistence with automatic SQLx migrations at startup
- S3-compatible object storage support, including MinIO, Cloudflare R2, Backblaze B2, and AWS S3
- Daily rotating application logs

## Requirements

- Rust 2024 toolchain
- PostgreSQL
- An S3-compatible bucket

## Configuration

readust loads configuration from environment variables prefixed with `READUST` and from an optional TOML configuration file passed with `--config`.

The easiest way to start is to copy the example file and edit it:

```sh
cp config.example.toml config.toml
```

For production, replace `jwt_secret`, database credentials, and S3 credentials. You can generate a JWT secret with:

```sh
openssl rand -hex 32
```

For private deployments, set:

```toml
[application]
disable_signup = true
```

after creating your first user.

## Database

Create a PostgreSQL database before starting the service:

```sql
CREATE DATABASE readust;
```

readust runs migrations automatically on startup. The initial migration creates tables for users, tokens, books, book configs, notes, and files.

For SQLx tooling or offline checks, `.example.env` shows the expected `DATABASE_URL` format:

```sh
DATABASE_URL=postgres://readust:password@localhost:5432/readust
```

## Running

Start the service with a config file:

```sh
cargo run -- --config config.toml
```

Build a release binary:

```sh
cargo build --release
./target/release/readust --config config.toml
```

## Auth Examples

Create a user:

```sh
curl -X POST http://localhost:8000/auth/v1/signup \
  -H 'content-type: application/json' \
  -d '{"email":"user@example.com","password":"password123"}'
```

Login:

```sh
curl -X POST 'http://localhost:8000/auth/v1/token?grant_type=password' \
  -H 'content-type: application/json' \
  -d '{"email":"user@example.com","password":"password123"}'
```

Get the current user:

```sh
curl http://localhost:8000/auth/v1/user \
  -H 'authorization: Bearer <access-token>'
```

## Storage Notes

Uploads return a presigned URL. The client uploads the object directly to S3 using that URL. File keys are stored with the authenticated user ID as a prefix:

```text
<user-id>/<file-name>
```

Path inputs are restricted to normal relative path components. Absolute paths and parent-directory components are rejected.

Temporary uploads are currently not supported, and storage quota enforcement is not implemented yet. The API returns an effectively unlimited quota for compatibility.

## Development

Run checks and tests:

```sh
cargo test
```

Format code:

```sh
cargo fmt
```

## License

See `LICENSE`.
