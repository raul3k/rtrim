# rtrim

A high-performance CLI tool for removing trailing whitespace from text files. Built in Rust with a focus on safety, atomicity, and zero external dependencies.

> **Note**: This is a learning project created to study Rust, file system operations, and safe system programming practices.

## Features

- **Atomic writes** - Uses write-sync-rename pattern to prevent data corruption
- **Permission preservation** - Maintains original file permissions after processing
- **Symlink protection** - Ignores symlinks to prevent security issues
- **Binary file detection** - Automatically skips non-UTF-8 files
- **Recursive processing** - Process entire directory trees
- **Zero dependencies** - Only uses Rust standard library

## Installation

### From source

```bash
# Clone the repository
git clone https://github.com/raul3k/rtrim.git
cd rtrim

# Build and install
make install
```

### Local installation (no sudo)

```bash
PREFIX=~/.local/bin make install
```

### Uninstall

```bash
make uninstall
```

## Usage

```bash
# Display help
rtrim --help

# Process a single file
rtrim --file path/to/file.txt

# Process a folder recursively
rtrim --folder path/to/folder
```

## How It Works

1. Reads the file content into memory
2. Validates UTF-8 encoding (skips binary files)
3. Removes trailing whitespace from each line
4. Writes to a unique temporary file
5. Syncs to disk (`fsync`)
6. Atomically renames temp file to original

This ensures that even during a power failure, you won't end up with a corrupted file.

## Ignored Directories

When processing folders recursively, the following are automatically skipped:

| Category | Directories |
|----------|-------------|
| Version Control | `.git`, `.svn`, `.hg` |
| Dependencies | `node_modules`, `target`, `__pycache__` |
| Virtual Environments | `.venv`, `venv` |
| IDEs | `.idea`, `.vscode` |
| Hidden | Any directory starting with `.` |

## Running Tests

```bash
cargo test
```

## Project Structure

```
rtrim/
├── src/
│   └── main.rs      # Main source code with unit tests
├── Cargo.toml       # Rust package manifest
├── Makefile         # Build and install automation
├── LICENSE          # MIT License
└── README.md
```

## Technical Details

- **Algorithm complexity**: O(N) where N is the file size
- **Memory usage**: Pre-allocates based on original file size
- **Temporary files**: Format `.{filename}.{pid}.{timestamp}.tmp`
- **Supported platforms**: Unix-like systems (Linux, macOS)

## License

MIT License - see [LICENSE](LICENSE) for details.

## Contributing

This project uses [Conventional Commits](https://www.conventionalcommits.org/).

Format: `<type>(<scope>): <description>`

Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

Examples:
- `feat: add recursive folder processing`
- `fix(parser): handle empty files`
- `docs: update installation instructions`

Git hooks are automatically configured on the first `cargo build`.

## Learning Topics Covered

This project was built to learn:

- Rust ownership and borrowing
- File system operations in Rust
- Atomic file operations (write-sync-rename pattern)
- Unix file permissions and metadata
- Symlink security considerations
- Unit and integration testing in Rust
- CLI argument parsing without external crates
