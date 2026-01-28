use std::process::Command;

fn main() {
    // Apenas configura hooks se estivermos em um repositório git
    if std::path::Path::new(".git").exists() {
        // Configura git para usar .githooks/ como diretório de hooks
        let _ = Command::new("git")
            .args(["config", "core.hooksPath", ".githooks"])
            .status();

        // Garante que o hook é executável
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(metadata) = std::fs::metadata(".githooks/commit-msg") {
                let mut perms = metadata.permissions();
                perms.set_mode(0o755);
                let _ = std::fs::set_permissions(".githooks/commit-msg", perms);
            }
        }
    }
}
