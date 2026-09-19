# Chiron Horizon

A cross-platform database manager for SQL, NoSQL, and vector databases, with a native ChironDB workspace, an AI assistant, and tools for exploring and managing data.

Chiron Horizon is an open-source database workbench for desktop and self-hosted web use.

Read the practical [Chiron Horizon User Guide](docs/content/docs/user-guide.mdx) for connections, query workspaces, AI, attachments, export/import, and MCP.

## Features

- **ChironDB workspace** — browse collections, execute ChironQL, inspect query results and traces, and review confirmations for destructive operations.
- **Multiple database engines** — work with MySQL, PostgreSQL, SQLite, Redis, MongoDB, DuckDB, SQL Server, ClickHouse, Oracle, Elasticsearch, and more. Additional engines are available through optional drivers.
- **Query editor** — syntax highlighting, metadata-aware SQL completion, formatting, query history, saved snippets, and selected query execution.
- **AI assistant** — generate and explain queries, troubleshoot errors, and review proposed actions with built-in execution safeguards.
- **Data tools** — browse and edit table data, import CSV and Excel files, export results, compare data, and transfer data between supported engines.
- **Schema tools** — inspect database objects, edit table structures, compare schemas, and open the context-aware Schema Viewer: ERDs for relational stores, metagraphs for graph metadata, declared JSON trees for document stores, and native summaries for vector and time-series stores.
- **Connectivity** — SSH tunnels, proxy settings, encrypted configuration export and import, and connection organization.
- **Desktop and web** — a Tauri desktop application for macOS, Windows, and Linux, plus a web backend for self-hosting.
- **CLI and MCP** — terminal workflows and AI agent integration for supported database connections.

Feature availability varies by database engine. The Schema Viewer uses only metadata declared by the connected engine; a schemaless collection without a validator or mapping is reported as such and is never inferred from sampled data.

## ChironDB

[ChironDB](https://github.com/Gaussian-id/ChironDB) is a vector database. Chiron Horizon provides a dedicated interface for working with its collections and query language.

1. Start your ChironDB server.
2. Create a connection in Chiron Horizon and select **ChironDB**.
3. Enter the server host, HTTP port (default: `7401`), and any required credentials.
4. Open the connection to browse collections or run ChironQL.

For ChironDB's relational preview, choose **ChironDB Relational (PostgreSQL wire)** under PostgreSQL and connect to port `7403` (default user `chiron`, database `gaussdb`, `sslmode=disable`). Horizon supports connection testing, SQL execution, joins, and table/column discovery through this profile. Index and foreign-key facets are reported as partial until ChironDB exposes compatible PostgreSQL catalog metadata.

For example:

```sql
SHOW COLLECTIONS;
```

The workspace displays query results, execution details, and optional traces returned by the server.

## Development

### Prerequisites

- Node.js **22.13.0 or newer**.
- pnpm **10.27.0**, as specified in `package.json`.
- Rust and Cargo; the desktop crate declares Rust **1.88.0** as its minimum. Use the toolchain pinned in the repository's CI workflows for reproducible builds.
- The native build tools and system libraries required by Tauri for your platform.

On Ubuntu/Debian, the desktop application requires these system packages:

```bash
sudo apt-get install -y build-essential pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev libappindicator3-dev librsvg2-dev patchelf libssl-dev
```

### Run the desktop app

```bash
git clone https://github.com/Gaussian-id/Chiron-Horizon.git Chiron-Horizon
cd Chiron-Horizon
pnpm install --frozen-lockfile
pnpm dev:tauri
```

With Make installed, `make` installs dependencies when needed and starts the desktop development environment. Use `make dev-fast` for the lighter development configuration with DuckDB sidecar support.

### Run the web app

Start the frontend and backend in separate terminals from the repository root:

```bash
# Terminal 1
pnpm dev:web
```

```bash
# Terminal 2
pnpm dev:backend
```

The repository also includes a [Docker build definition](deploy/Dockerfile) and a [Compose configuration](deploy/docker-compose.yml) for building the web application from source. Review the deployment settings and configure your own login password before use.

### Build the desktop app

```bash
pnpm tauri build
```

Tauri prints the generated bundle paths when the build completes.

### Project checks

```bash
pnpm check
pnpm test
```

For Rust checks with the lighter feature configuration, use `make cargo-check-fast` and `make cargo-test-fast`.

## Project structure

| Directory | Purpose |
| --- | --- |
| `apps/desktop/` | Vue and TypeScript application interface |
| `src-tauri/` | Tauri desktop application |
| `crates/` | Rust database core, web backend, CLI, and MCP server |
| `plugins/` | Connection profiles, SQL dialects, and plugin tooling |
| `agents/` | Optional database driver processes |
| `deploy/` | Deployment configuration and database test environments |
| `docs/` | Documentation source |

Package names, configuration keys, and documentation use the Chiron Horizon identity.

## Contributing

Report bugs and request features in [this repository's issue tracker](https://github.com/Gaussian-id/Chiron-Horizon/issues). Include the application version, operating system, database engine, and steps to reproduce when reporting a problem.

Contributions are welcome through [pull requests](https://github.com/Gaussian-id/Chiron-Horizon/pulls). Include a description of the change and the checks you ran.

## Acknowledgments

Chiron Horizon is a fork of [DBX](https://github.com/t8y2/dbx), originally created by [t8y2](https://github.com/t8y2) and its contributors. We thank the upstream maintainers and community for the database tooling and architecture that form the foundation of this project.

Chiron Horizon adds ChironDB integration and its own branding and interface. Upstream copyright and license notices are retained.

## License

[Apache License 2.0](LICENSE).
