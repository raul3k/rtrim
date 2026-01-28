use std::env;
use std::fs::{self, File, Metadata, Permissions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process;
use std::time::SystemTime;

/// Defines the operation mode and target path.
#[derive(Debug)]
struct Config {
    mode: Mode,
    path: PathBuf,
    verbose: bool,
}

#[derive(Debug, PartialEq)]
enum Mode {
    File,
    Folder,
    Help,
}

/// Directories to be ignored during recursive traversal.
const IGNORED_DIRS: &[&str] = &[
    ".git",
    ".svn",
    ".hg",
    "node_modules",
    "target",
    "__pycache__",
    ".venv",
    "venv",
    ".idea",
    ".vscode",
];

fn main() {
    let args: Vec<String> = env::args().collect();

    let config = parse_config(&args).unwrap_or_else(|err| {
        eprintln!("Configuration Error: {}", err);
        process::exit(1);
    });

    if let Err(e) = run(config) {
        eprintln!("Execution Error: {}", e);
        process::exit(1);
    }
}

/// Displays the help message.
fn print_help() {
    let help = r#"rtrim - Atomic trailing whitespace remover

USAGE:
    rtrim --file <path>       Process a single file
    rtrim --folder <path>     Process a folder recursively
    rtrim --help              Display this help message

DESCRIPTION:
    Removes trailing whitespace (spaces, tabs, etc.) from the end of each
    line in text files. Binary files are automatically detected and ignored.

SECURITY:
    - Atomic writes via write-sync-rename
    - Preserves original file permissions
    - Ignores symlinks to prevent attacks
    - Uses unique temporary file names

OPTIONS:
    -v, --verbose         Show detailed processing information

IGNORED DIRECTORIES:
    .git, .svn, .hg, node_modules, target, __pycache__,
    .venv, venv, .idea, .vscode

EXAMPLES:
    rtrim --file src/main.rs
    rtrim --folder ./src
    rtrim --folder ./src --verbose
"#;
    println!("{}", help);
}

/// Performs manual CLI argument parsing.
fn parse_config(args: &[String]) -> Result<Config, &'static str> {
    if args.len() < 2 {
        return Err("Usage: rtrim --file <path> | rtrim --folder <path> | rtrim --help");
    }

    // Check for verbose flag anywhere in args
    let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");

    // Filter out verbose flags for mode parsing
    let filtered_args: Vec<&String> = args
        .iter()
        .filter(|a| *a != "--verbose" && *a != "-v")
        .collect();

    if filtered_args.len() < 2 {
        return Err("Usage: rtrim --file <path> | rtrim --folder <path> | rtrim --help");
    }

    match filtered_args[1].as_str() {
        "--help" | "-h" => Ok(Config {
            mode: Mode::Help,
            path: PathBuf::new(),
            verbose,
        }),
        "--file" => {
            if filtered_args.len() < 3 {
                return Err("Usage: rtrim --file <path>");
            }
            Ok(Config {
                mode: Mode::File,
                path: PathBuf::from(filtered_args[2]),
                verbose,
            })
        }
        "--folder" => {
            if filtered_args.len() < 3 {
                return Err("Usage: rtrim --folder <path>");
            }
            Ok(Config {
                mode: Mode::Folder,
                path: PathBuf::from(filtered_args[2]),
                verbose,
            })
        }
        _ => Err("Invalid flag. Use --file, --folder, or --help."),
    }
}

fn run(config: Config) -> io::Result<()> {
    match config.mode {
        Mode::Help => {
            print_help();
            Ok(())
        }
        Mode::File => {
            // Check if it's a symlink before processing
            let metadata = fs::symlink_metadata(&config.path)?;
            if metadata.file_type().is_symlink() {
                if config.verbose {
                    println!("  Skipped (symlink): {:?}", config.path);
                } else {
                    eprintln!("Warning: Ignoring symlink {:?}", config.path);
                }
                return Ok(());
            }
            process_file(&config.path, config.verbose)
        }
        Mode::Folder => process_folder(&config.path, config.verbose),
    }
}

/// Checks if a directory should be ignored.
fn should_ignore_dir(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| IGNORED_DIRS.contains(&name) || name.starts_with('.'))
        .unwrap_or(false)
}

/// Recursive filesystem traversal (without following symlinks).
fn process_folder(dir: &Path, verbose: bool) -> io::Result<()> {
    // Use symlink_metadata to avoid following symlinks
    let metadata = fs::symlink_metadata(dir)?;

    // Ignore symlinks
    if metadata.file_type().is_symlink() {
        if verbose {
            println!("  Skipped (symlink): {:?}", dir);
        }
        return Ok(());
    }

    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Path is not a directory",
        ));
    }

    if verbose {
        println!("Scanning: {:?}", dir);
    }

    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();

        // Use symlink_metadata to detect symlinks without following them
        let entry_metadata = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Warning: Could not read metadata for {:?}: {}", path, e);
                continue;
            }
        };

        // Ignore symlinks completely
        if entry_metadata.file_type().is_symlink() {
            if verbose {
                println!("  Skipped (symlink): {:?}", path);
            }
            continue;
        }

        if entry_metadata.is_dir() {
            // Ignore special directories
            if should_ignore_dir(&path) {
                if verbose {
                    println!("  Skipped (ignored dir): {:?}", path);
                }
                continue;
            }
            process_folder(&path, verbose)?;
        } else if entry_metadata.is_file() {
            if let Err(e) = process_file(&path, verbose) {
                eprintln!("Warning: Error processing {:?}: {}", path, e);
            }
        }
    }

    Ok(())
}

/// Generates a unique temporary file name in the same directory.
fn generate_temp_path(original: &Path) -> PathBuf {
    let parent = original.parent().unwrap_or(Path::new("."));
    let timestamp = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let pid = process::id();

    let original_name = original
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("file");

    let temp_name = format!(".{}.{}.{}.tmp", original_name, pid, timestamp);
    parent.join(temp_name)
}

/// Applies the original file permissions to the new file.
fn preserve_permissions(temp_path: &Path, original_metadata: &Metadata) -> io::Result<()> {
    let permissions = Permissions::from_mode(original_metadata.mode());
    fs::set_permissions(temp_path, permissions)?;
    Ok(())
}

/// Result of trimming operation.
#[derive(Debug, PartialEq)]
struct TrimResult {
    content: String,
    modified: bool,
}

/// Removes trailing whitespace from each line of the input.
/// Returns the trimmed content and whether any modifications were made.
fn trim_trailing_whitespace(content: &str) -> TrimResult {
    let mut output = String::with_capacity(content.len());
    let mut modified = false;

    for line in content.lines() {
        let trimmed = line.trim_end();
        if trimmed.len() != line.len() {
            modified = true;
        }
        output.push_str(trimmed);
        output.push('\n');
    }

    // Preserve original behavior: if file didn't end with newline, remove the added one
    if !content.ends_with('\n') && !output.is_empty() {
        output.pop();
    }

    TrimResult { content: output, modified }
}

/// Individual file processing with security validation and atomicity.
fn process_file(path: &Path, verbose: bool) -> io::Result<()> {
    // Double-check it's not a symlink (defense in depth)
    let original_metadata = fs::symlink_metadata(path)?;
    if original_metadata.file_type().is_symlink() {
        if verbose {
            println!("  Skipped (symlink): {:?}", path);
        }
        return Ok(());
    }

    if !original_metadata.is_file() {
        return Ok(());
    }

    if verbose {
        println!("  Checking: {:?}", path);
    }

    let mut buffer = Vec::new();
    {
        let mut file = File::open(path)?;
        file.read_to_end(&mut buffer)?;
    }

    // Binary file protection via UTF-8 validation.
    let content = match std::str::from_utf8(&buffer) {
        Ok(s) => s,
        Err(_) => {
            if verbose {
                println!("  Skipped (binary): {:?}", path);
            }
            return Ok(());
        }
    };

    let result = trim_trailing_whitespace(content);

    if result.modified {
        // Generate unique temp name (prevents collisions and symlink attacks)
        let temp_path = generate_temp_path(path);

        // Check if temp file already exists (shouldn't, but for safety)
        if temp_path.exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("Temporary file already exists: {:?}", temp_path),
            ));
        }

        {
            let mut temp_file = File::create(&temp_path)?;
            temp_file.write_all(result.content.as_bytes())?;
            temp_file.sync_all()?;
        }

        // Preserve original file permissions
        if let Err(e) = preserve_permissions(&temp_path, &original_metadata) {
            // If permission preservation fails, remove temp file and propagate error
            let _ = fs::remove_file(&temp_path);
            return Err(e);
        }

        // Atomic rename
        if let Err(e) = fs::rename(&temp_path, path) {
            // If rename fails, remove temp file
            let _ = fs::remove_file(&temp_path);
            return Err(e);
        }

        println!("  Processed: {:?}", path);
    } else if verbose {
        println!("  Unchanged: {:?}", path);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn create_test_dir() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!(
            "rtrim_test_{}_{}_{}",
            process::id(),
            counter,
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn cleanup_test_dir(dir: &Path) {
        let _ = fs::remove_dir_all(dir);
    }

    // ==================== Argument Parsing Tests ====================

    #[test]
    fn test_parse_config_help_long() {
        let args = vec!["rtrim".to_string(), "--help".to_string()];
        let config = parse_config(&args).unwrap();
        assert_eq!(config.mode, Mode::Help);
    }

    #[test]
    fn test_parse_config_help_short() {
        let args = vec!["rtrim".to_string(), "-h".to_string()];
        let config = parse_config(&args).unwrap();
        assert_eq!(config.mode, Mode::Help);
    }

    #[test]
    fn test_parse_config_file_mode() {
        let args = vec![
            "rtrim".to_string(),
            "--file".to_string(),
            "test.txt".to_string(),
        ];
        let config = parse_config(&args).unwrap();
        assert_eq!(config.mode, Mode::File);
        assert_eq!(config.path, PathBuf::from("test.txt"));
    }

    #[test]
    fn test_parse_config_folder_mode() {
        let args = vec![
            "rtrim".to_string(),
            "--folder".to_string(),
            "./src".to_string(),
        ];
        let config = parse_config(&args).unwrap();
        assert_eq!(config.mode, Mode::Folder);
        assert_eq!(config.path, PathBuf::from("./src"));
    }

    #[test]
    fn test_parse_config_no_args() {
        let args = vec!["rtrim".to_string()];
        let result = parse_config(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_config_invalid_flag() {
        let args = vec!["rtrim".to_string(), "--invalid".to_string()];
        let result = parse_config(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid flag"));
    }

    #[test]
    fn test_parse_config_file_missing_path() {
        let args = vec!["rtrim".to_string(), "--file".to_string()];
        let result = parse_config(&args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_config_folder_missing_path() {
        let args = vec!["rtrim".to_string(), "--folder".to_string()];
        let result = parse_config(&args);
        assert!(result.is_err());
    }

    // ==================== Trim Logic Tests ====================

    #[test]
    fn test_trim_trailing_spaces() {
        let input = "hello world   \n";
        let result = trim_trailing_whitespace(input);
        assert_eq!(result.content, "hello world\n");
        assert!(result.modified);
    }

    #[test]
    fn test_trim_trailing_tabs() {
        let input = "hello world\t\t\n";
        let result = trim_trailing_whitespace(input);
        assert_eq!(result.content, "hello world\n");
        assert!(result.modified);
    }

    #[test]
    fn test_trim_mixed_whitespace() {
        let input = "hello world \t \t\n";
        let result = trim_trailing_whitespace(input);
        assert_eq!(result.content, "hello world\n");
        assert!(result.modified);
    }

    #[test]
    fn test_trim_multiple_lines() {
        let input = "line1   \nline2\t\nline3 \t \n";
        let result = trim_trailing_whitespace(input);
        assert_eq!(result.content, "line1\nline2\nline3\n");
        assert!(result.modified);
    }

    #[test]
    fn test_trim_no_changes_needed() {
        let input = "hello world\nno trailing\n";
        let result = trim_trailing_whitespace(input);
        assert_eq!(result.content, "hello world\nno trailing\n");
        assert!(!result.modified);
    }

    #[test]
    fn test_trim_preserves_no_final_newline() {
        let input = "no newline at end   ";
        let result = trim_trailing_whitespace(input);
        assert_eq!(result.content, "no newline at end");
        assert!(result.modified);
    }

    #[test]
    fn test_trim_preserves_final_newline() {
        let input = "has newline   \n";
        let result = trim_trailing_whitespace(input);
        assert_eq!(result.content, "has newline\n");
        assert!(result.modified);
    }

    #[test]
    fn test_trim_empty_file() {
        let input = "";
        let result = trim_trailing_whitespace(input);
        assert_eq!(result.content, "");
        assert!(!result.modified);
    }

    #[test]
    fn test_trim_only_newline() {
        let input = "\n";
        let result = trim_trailing_whitespace(input);
        assert_eq!(result.content, "\n");
        assert!(!result.modified);
    }

    #[test]
    fn test_trim_only_whitespace_line() {
        let input = "   \n";
        let result = trim_trailing_whitespace(input);
        assert_eq!(result.content, "\n");
        assert!(result.modified);
    }

    #[test]
    fn test_trim_multiple_empty_lines() {
        let input = "text\n\n\n";
        let result = trim_trailing_whitespace(input);
        assert_eq!(result.content, "text\n\n\n");
        assert!(!result.modified);
    }

    #[test]
    fn test_trim_preserves_leading_whitespace() {
        let input = "    indented line   \n";
        let result = trim_trailing_whitespace(input);
        assert_eq!(result.content, "    indented line\n");
        assert!(result.modified);
    }

    #[test]
    fn test_trim_carriage_return() {
        // Note: Rust's lines() treats \r\n as a line separator,
        // so \r is not part of the line content to be trimmed.
        // This test verifies that CRLF files are handled correctly.
        let input = "windows line\r\n";
        let result = trim_trailing_whitespace(input);
        // lines() already strips \r from line endings
        assert_eq!(result.content, "windows line\n");
        // No trailing whitespace in the line itself, so not modified
        assert!(!result.modified);
    }

    #[test]
    fn test_trim_crlf_with_trailing_spaces() {
        // CRLF with actual trailing spaces before \r\n
        let input = "windows line   \r\n";
        let result = trim_trailing_whitespace(input);
        assert_eq!(result.content, "windows line\n");
        assert!(result.modified);
    }

    // ==================== Directory Ignore Tests ====================

    #[test]
    fn test_should_ignore_git() {
        assert!(should_ignore_dir(Path::new("/project/.git")));
    }

    #[test]
    fn test_should_ignore_node_modules() {
        assert!(should_ignore_dir(Path::new("/project/node_modules")));
    }

    #[test]
    fn test_should_ignore_target() {
        assert!(should_ignore_dir(Path::new("/project/target")));
    }

    #[test]
    fn test_should_ignore_idea() {
        assert!(should_ignore_dir(Path::new("/project/.idea")));
    }

    #[test]
    fn test_should_ignore_vscode() {
        assert!(should_ignore_dir(Path::new("/project/.vscode")));
    }

    #[test]
    fn test_should_ignore_hidden_dirs() {
        assert!(should_ignore_dir(Path::new("/project/.hidden")));
        assert!(should_ignore_dir(Path::new("/project/.config")));
    }

    #[test]
    fn test_should_not_ignore_src() {
        assert!(!should_ignore_dir(Path::new("/project/src")));
    }

    #[test]
    fn test_should_not_ignore_regular_dir() {
        assert!(!should_ignore_dir(Path::new("/project/lib")));
    }

    // ==================== Temp Path Generation Tests ====================

    #[test]
    fn test_generate_temp_path_format() {
        let original = Path::new("/tmp/test.txt");
        let temp = generate_temp_path(original);

        let temp_name = temp.file_name().unwrap().to_str().unwrap();
        assert!(temp_name.starts_with(".test.txt."));
        assert!(temp_name.ends_with(".tmp"));
        assert_eq!(temp.parent().unwrap(), Path::new("/tmp"));
    }

    #[test]
    fn test_generate_temp_path_uniqueness() {
        let original = Path::new("/tmp/test.txt");
        let temp1 = generate_temp_path(original);
        std::thread::sleep(std::time::Duration::from_nanos(1));
        let temp2 = generate_temp_path(original);

        // Should generate different names due to timestamp
        assert_ne!(temp1, temp2);
    }

    #[test]
    fn test_generate_temp_path_hidden() {
        let original = Path::new("/tmp/test.txt");
        let temp = generate_temp_path(original);

        let temp_name = temp.file_name().unwrap().to_str().unwrap();
        assert!(temp_name.starts_with('.'), "Temp file should be hidden");
    }

    // ==================== Integration Tests ====================

    #[test]
    fn test_process_file_removes_trailing_spaces() {
        let test_dir = create_test_dir();
        let test_file = test_dir.join("test.txt");

        fs::write(&test_file, "hello   \nworld\t\n").unwrap();
        process_file(&test_file, false).unwrap();

        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, "hello\nworld\n");

        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_process_file_preserves_permissions() {
        let test_dir = create_test_dir();
        let test_file = test_dir.join("test.txt");

        fs::write(&test_file, "hello   \n").unwrap();
        fs::set_permissions(&test_file, Permissions::from_mode(0o754)).unwrap();

        process_file(&test_file, false).unwrap();

        let metadata = fs::metadata(&test_file).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o754);

        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_process_file_ignores_binary() {
        let test_dir = create_test_dir();
        let test_file = test_dir.join("binary.bin");

        // Write invalid UTF-8
        fs::write(&test_file, &[0xFF, 0xFE, 0x00, 0x01]).unwrap();
        let original_content = fs::read(&test_file).unwrap();

        process_file(&test_file, false).unwrap();

        let new_content = fs::read(&test_file).unwrap();
        assert_eq!(original_content, new_content);

        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_process_file_no_changes_when_clean() {
        let test_dir = create_test_dir();
        let test_file = test_dir.join("clean.txt");

        let original = "hello\nworld\n";
        fs::write(&test_file, original).unwrap();
        let mtime_before = fs::metadata(&test_file).unwrap().modified().unwrap();

        // Small delay to ensure mtime would change if file is rewritten
        std::thread::sleep(std::time::Duration::from_millis(10));

        process_file(&test_file, false).unwrap();

        let mtime_after = fs::metadata(&test_file).unwrap().modified().unwrap();
        // File should not have been modified
        assert_eq!(mtime_before, mtime_after);

        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_process_file_ignores_symlink() {
        let test_dir = create_test_dir();
        let original_file = test_dir.join("original.txt");
        let symlink_file = test_dir.join("symlink.txt");

        fs::write(&original_file, "hello   \n").unwrap();
        std::os::unix::fs::symlink(&original_file, &symlink_file).unwrap();

        process_file(&symlink_file, false).unwrap();

        // Original file should not have been modified
        let content = fs::read_to_string(&original_file).unwrap();
        assert_eq!(content, "hello   \n");

        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_process_folder_recursive() {
        let test_dir = create_test_dir();
        let sub_dir = test_dir.join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let file1 = test_dir.join("file1.txt");
        let file2 = sub_dir.join("file2.txt");

        fs::write(&file1, "line1   \n").unwrap();
        fs::write(&file2, "line2\t\n").unwrap();

        process_folder(&test_dir, false).unwrap();

        assert_eq!(fs::read_to_string(&file1).unwrap(), "line1\n");
        assert_eq!(fs::read_to_string(&file2).unwrap(), "line2\n");

        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_process_folder_ignores_git() {
        let test_dir = create_test_dir();
        let git_dir = test_dir.join(".git");
        fs::create_dir(&git_dir).unwrap();

        let git_file = git_dir.join("config");
        fs::write(&git_file, "content   \n").unwrap();

        process_folder(&test_dir, false).unwrap();

        // File inside .git should not have been modified
        let content = fs::read_to_string(&git_file).unwrap();
        assert_eq!(content, "content   \n");

        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_no_temp_file_left_on_success() {
        let test_dir = create_test_dir();
        let test_file = test_dir.join("test.txt");

        fs::write(&test_file, "hello   \n").unwrap();
        process_file(&test_file, false).unwrap();

        // Check no .tmp files are left
        let entries: Vec<_> = fs::read_dir(&test_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().map(|ext| ext == "tmp").unwrap_or(false))
            .collect();

        assert!(entries.is_empty(), "No temp files should remain");

        cleanup_test_dir(&test_dir);
    }

    // ==================== Verbose Flag Tests ====================

    #[test]
    fn test_parse_config_verbose_long() {
        let args = vec![
            "rtrim".to_string(),
            "--file".to_string(),
            "test.txt".to_string(),
            "--verbose".to_string(),
        ];
        let config = parse_config(&args).unwrap();
        assert_eq!(config.mode, Mode::File);
        assert!(config.verbose);
    }

    #[test]
    fn test_parse_config_verbose_short() {
        let args = vec![
            "rtrim".to_string(),
            "--folder".to_string(),
            "./src".to_string(),
            "-v".to_string(),
        ];
        let config = parse_config(&args).unwrap();
        assert_eq!(config.mode, Mode::Folder);
        assert!(config.verbose);
    }

    #[test]
    fn test_parse_config_verbose_before_mode() {
        let args = vec![
            "rtrim".to_string(),
            "--verbose".to_string(),
            "--file".to_string(),
            "test.txt".to_string(),
        ];
        let config = parse_config(&args).unwrap();
        assert_eq!(config.mode, Mode::File);
        assert!(config.verbose);
    }

    #[test]
    fn test_parse_config_no_verbose() {
        let args = vec![
            "rtrim".to_string(),
            "--file".to_string(),
            "test.txt".to_string(),
        ];
        let config = parse_config(&args).unwrap();
        assert!(!config.verbose);
    }

    #[test]
    fn test_process_file_verbose_mode() {
        let test_dir = create_test_dir();
        let test_file = test_dir.join("test.txt");

        fs::write(&test_file, "hello   \n").unwrap();
        // Should not panic in verbose mode
        process_file(&test_file, true).unwrap();

        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, "hello\n");

        cleanup_test_dir(&test_dir);
    }

    #[test]
    fn test_process_folder_verbose_mode() {
        let test_dir = create_test_dir();
        let test_file = test_dir.join("test.txt");

        fs::write(&test_file, "hello   \n").unwrap();
        // Should not panic in verbose mode
        process_folder(&test_dir, true).unwrap();

        let content = fs::read_to_string(&test_file).unwrap();
        assert_eq!(content, "hello\n");

        cleanup_test_dir(&test_dir);
    }
}
