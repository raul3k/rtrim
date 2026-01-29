# Security Policy

## Supported Versions

| Version | Supported          |
| ------- | ------------------ |
| 0.1.x   | :white_check_mark: |

## Reporting a Vulnerability

If you discover a security vulnerability in rtrim, please report it responsibly:

1. **Do not** open a public issue
2. Use [GitHub's private vulnerability reporting](https://github.com/raul3k/rtrim/security/advisories/new) to submit your report
3. Include in your report:
   - Description of the vulnerability
   - Steps to reproduce
   - Potential impact
   - Any suggested fixes (optional)

We will acknowledge receipt and provide updates on the fix timeline.

## Security Considerations

rtrim is designed with security in mind:

- **Symlink protection**: Symlinks are ignored to prevent path traversal attacks
- **Atomic writes**: Uses write-sync-rename pattern to prevent data corruption
- **Permission preservation**: Original file permissions are maintained
- **Binary detection**: Non-UTF-8 files are automatically skipped

## Scope

This security policy applies to the rtrim CLI tool. As this is a learning project, response times may vary.
