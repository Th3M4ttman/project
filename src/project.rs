use crate::template;
use crate::utils;
use anyhow::{Result, anyhow};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn find_project_path(name: &str) -> Option<PathBuf> {
    let projects_dir = dirs::home_dir()?.join("projects");
    let mut found = None;

    // Look for a folder matching the name
    if let Ok(entries) = fs::read_dir(&projects_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.file_name().and_then(|f| f.to_str()) == Some(name) {
                let proj_file = path.join(".proj/project.json");
                if proj_file.is_file() {
                    found = Some(path);
                    break;
                }
            }
        }
    }

    found
}

/// Return the central projects directory (`~/projects`)
pub fn projects_dir() -> PathBuf {
    dirs::home_dir().unwrap().join("projects")
}

/// Make sure `~/projects` exists
pub fn ensure_projects_dir() -> std::io::Result<()> {
    let dir = projects_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(())
}

/// Create a symlink in `~/projects` if project is outside of it
pub fn link_in_projects_dir(project_path: &Path) {
    let projects = projects_dir();
    let proj_name = project_path.file_name().unwrap_or_default();
    let symlink_path = projects.join(proj_name);

    if symlink_path.exists() {
        return;
    }

    #[cfg(unix)]
    std::os::unix::fs::symlink(project_path, &symlink_path)
        .unwrap_or_else(|e| eprintln!("Failed to create symlink: {}", e));

    #[cfg(windows)]
    std::os::windows::fs::symlink_dir(project_path, &symlink_path)
        .unwrap_or_else(|e| eprintln!("Failed to create symlink: {}", e));
}

pub fn maybe_create_upstream(project_name: &str, project_path: &Path) {
    println!(
        "Do you want to create a GitHub repository for '{}' and push the current branch? [y/N]: ",
        project_name
    );
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    let trimmed = input.trim().to_lowercase();

    if trimmed == "y" || trimmed == "yes" {
        let status = Command::new("gh")
            .args(["repo", "create"])
            .current_dir(project_path)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status();

        match status {
            Ok(s) if s.success() => println!("✅ GitHub repo created and pushed!"),
            Ok(s) => eprintln!("❌ Failed to create repo, exit code {}", s),
            Err(e) => eprintln!("❌ Failed to run `gh`: {}", e),
        }
    }
}

/// Initialize a new .proj folder and Git repo
pub fn init_project(interactive: bool, template: Option<String>, vars: &[(String, String)]) {
    let current_dir = env::current_dir().expect("Failed to get current directory");
    let proj_name = current_dir
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let proj_dir = current_dir.join(".proj");

    if !proj_dir.exists() {
        fs::create_dir_all(&proj_dir).expect("Failed to create .proj directory");

        let default_json = json!({
            "name": proj_name,
            "version": "0.1.0",
            "description": "New project",
            "template": null,
            "status": "active",
            "completion": 0.0
        });

        fs::write(
            proj_dir.join("project.json"),
            serde_json::to_string_pretty(&default_json).unwrap(),
        )
        .expect("Failed to write project.json");

        println!("✅ Initialized project '{}'", proj_name);
    } else {
        println!(".proj already exists.");
    }

    init_git_repo(&current_dir);
    maybe_create_upstream(&proj_name, &current_dir);
    let proj_file = proj_dir.join("project.json");
    let mut json_data = utils::read_json(&proj_file);

    for (k, v) in vars {
        json_data[k] = Value::String(v.clone());
    }

    if json_data.get("template").and_then(|v| v.as_str()).is_none() {
        let chosen_template = template.or_else(template::select_template);
        if let Some(t) = chosen_template {
            template::apply_boilr_template(&t, &proj_file, interactive);
            json_data["template"] = Value::String(t);
        }
    }

    fs::write(
        &proj_file,
        serde_json::to_string_pretty(&json_data).unwrap(),
    )
    .expect("Failed to update project.json");

    // Link project in ~/projects if outside
    if !current_dir.starts_with(projects_dir()) {
        link_in_projects_dir(&current_dir);
    }
    // After applying the Boilr template
    if Command::new("git")
        .arg("rev-parse")
        .arg("--is-inside-work-tree")
        .current_dir(&current_dir)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        // Stage all files
        let _ = Command::new("git")
            .arg("add")
            .arg("-A")
            .current_dir(&current_dir)
            .status();

        // Commit
        let _ = Command::new("git")
            .arg("commit")
            .arg("-m")
            .arg("initial commit")
            .current_dir(&current_dir)
            .status();

        // Push and set upstream
        let _ = Command::new("git")
            .arg("push")
            .arg("--set-upstream")
            .arg("origin")
            .arg("master")
            .current_dir(&current_dir)
            .status();
    }
}

/// Create a new project directory
pub fn create_project(
    name: &str,
    template: Option<String>,
    vars: &[(String, String)],
    interactive: bool,
) {
    let path = Path::new(name)
        .canonicalize()
        .unwrap_or_else(|_| Path::new(name).to_path_buf());
    if path.exists() {
        eprintln!("Error: directory '{}' already exists.", name);
        return;
    }

    fs::create_dir_all(&path).expect("Failed to create project directory");
    env::set_current_dir(&path).expect("Failed to change directory");

    init_project(interactive, template, vars);

    // Link in ~/projects if outside
    if !path.starts_with(projects_dir()) {
        link_in_projects_dir(&path);
    }

    println!("📁 Created new project '{}'", name);
}

pub fn set_project_vars(vars: &[(String, String)]) {
    let proj_file = Path::new(".proj/project.json");
    let mut data = utils::read_json(proj_file);

    for (key, value) in vars {
        if key == "completion" {
            if let Ok(f) = value.parse::<f64>() {
                data[key] = serde_json::json!(f);
                continue;
            }
        }
        data[key] = Value::String(value.clone());
    }

    fs::write(proj_file, serde_json::to_string_pretty(&data).unwrap())
        .expect("Failed to write project.json");

    println!("✅ Updated project.json");
}

pub fn get_project_var(key: &str) {
    let proj_file = Path::new(".proj/project.json");
    let data = utils::read_json(proj_file);

    match data.get(key) {
        Some(val) => println!("{}", val),
        None => eprintln!("Key '{}' not found.", key),
    }
}

pub fn init_git_repo(path: &Path) {
    if path.join(".git").exists() {
        return;
    }
    let _ = Command::new("git").arg("init").current_dir(path).output();
}

pub fn scan_for_proj(recursive: bool) {
    ensure_projects_dir().ok();

    let mut seen = HashSet::new();

    fn visit(dir: &Path, recursive: bool, seen: &mut HashSet<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.join(".proj").exists() {
                    // Use canonical path to deduplicate symlinks
                    if let Ok(real_path) = fs::canonicalize(&path) {
                        if seen.insert(real_path) {
                            println!(
                                "Found project: {}",
                                path.file_name().unwrap_or_default().to_string_lossy()
                            );
                        }
                    }
                }

                if recursive && path.is_dir() {
                    visit(&path, recursive, seen);
                }
            }
        }
    }

    // Scan current directory
    visit(Path::new("."), recursive, &mut seen);

    // Scan ~/projects/
    visit(&projects_dir(), recursive, &mut seen);
}

pub fn git_status_flags(path: &Path) -> (bool, bool, bool) {
    use std::process::Command;

    // Untracked / unadded files
    let unadded = Command::new("git")
        .arg("ls-files")
        .arg("--others")
        .arg("--exclude-standard")
        .current_dir(path)
        .output()
        .map(|o| !o.stdout.is_empty())
        .unwrap_or(false);

    // Uncommitted changes (staged or unstaged)
    let uncommitted = Command::new("git")
        .arg("diff")
        .arg("--quiet")
        .current_dir(path)
        .status()
        .map(|s| !s.success())
        .unwrap_or(false)
        || Command::new("git")
            .arg("diff")
            .arg("--cached")
            .arg("--quiet")
            .current_dir(path)
            .status()
            .map(|s| !s.success())
            .unwrap_or(false);

    // Unpushed commits (only if remote exists)
    let unpushed = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "@{u}"])
        .current_dir(path)
        .output()
        .map(|o| o.status.success()) // only run if upstream exists
        .unwrap_or(false)
        && Command::new("git")
            .args(["log", "@{u}..HEAD", "--oneline"])
            .current_dir(path)
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false);

    (unadded, uncommitted, unpushed)
}

pub fn list_projects(status_filter: &str, show_progress: bool) {
    ensure_projects_dir().ok();

    let mut seen = std::collections::HashSet::new();

    /// Recursively scan directories for projects
    fn visit(
        dir: &Path,
        recursive: bool,
        seen: &mut std::collections::HashSet<PathBuf>,
    ) -> Vec<PathBuf> {
        let mut projects = Vec::new();

        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();

                // Skip non-directories
                if !path.is_dir() {
                    continue;
                }

                // Skip hidden folders
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if name.starts_with('.') {
                        continue;
                    }
                }

                let proj_file = path.join(".proj/project.json");
                if proj_file.is_file() {
                    if let Ok(real_path) = fs::canonicalize(&path) {
                        if seen.insert(real_path.clone()) {
                            projects.push(real_path);
                        }
                    }
                } else if recursive {
                    projects.extend(visit(&path, recursive, seen));
                }
            }
        }

        projects
    }

    // Scan current directory and ~/projects
    let mut all_projects = visit(Path::new("."), true, &mut seen);
    all_projects.extend(visit(&projects_dir(), true, &mut seen));

    for project_path in all_projects {
        let proj_file = project_path.join(".proj/project.json");
        if !proj_file.is_file() {
            continue; // safety check
        }

        let data = utils::read_json(&proj_file);

        let status = data
            .get("status")
            .and_then(|v| v.as_str())
            .unwrap_or("active");
        let completion = data
            .get("completion")
            .and_then(|v| v.as_f64())
            .unwrap_or(0.0);

        if status_filter != "all" && status != status_filter {
            continue;
        }

        // Git flags only if .git exists
        let (unadded, uncommitted, unpushed) = if project_path.join(".git").exists() {
            git_status_flags(&project_path)
        } else {
            (false, false, false)
        };

        let mut flags = String::new();
        if unadded {
            flags.push_str("\x1b[31m+\x1b[0m");
        } // Use \x1b for escape sequences
        if uncommitted {
            flags.push_str("\x1b[31mc\x1b[0m");
        }
        if unpushed {
            flags.push_str("\x1b[31m^\x1b[0m");
        }

        let project_name = project_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();

        if show_progress {
            let bar_len = 20;
            let filled = (completion * bar_len as f64).round() as usize;
            let empty = bar_len - filled;

            let color = if completion < 0.33 {
                "\x1b[31m" // red
            } else if completion < 0.66 {
                "\x1b[33m" // yellow
            } else {
                "\x1b[32m" // green
            };

            let bar = format!(
                "{}{}{}\x1b[0m{}",
                color,
                "█".repeat(filled),
                "\x1b[0m",
                "░".repeat(empty)
            );

            println!(
                "{} {} [{}] {:.0}%",
                project_name,
                flags,
                bar,
                completion * 100.0
            );
        } else {
            println!(
                "{} {} (status: {}, completion: {:.0}%)",
                project_name,
                flags,
                status,
                completion * 100.0
            );
        }
    }
}

pub fn migrate_project(name: &str, destination: Option<PathBuf>) -> Result<()> {
    let default_dest = dirs::home_dir().unwrap().join("projects");
    let destination = destination.unwrap_or(default_dest);
    let dest_path = destination.join(name);

    // First try the registered project path
    let project_path = find_project_path(name)
        // fallback: check if a directory exists in cwd or home
        .or_else(|| {
            let cwd_path = std::env::current_dir().ok()?.join(name);
            if cwd_path.exists() {
                Some(cwd_path)
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow!("Project '{}' not found", name))?;

    let real_path = fs::canonicalize(&project_path)?;

    if dest_path.exists() {
        anyhow::bail!("Destination already contains a project named '{}'", name);
    }

    fs::create_dir_all(&destination)?;
    fs::rename(&real_path, &dest_path)?;

    // Remove old symlink if it exists
    if project_path.exists() && project_path.is_symlink() {
        fs::remove_file(&project_path)?;
    }

    println!(
        "✅ Project '{}' migrated to '{}'",
        name,
        dest_path.display()
    );
    Ok(())
}

pub fn remove_project(name: &str, force: bool) -> anyhow::Result<()> {
    use anyhow::{Context, anyhow};
    use std::io::{self, Write};

    let projects_dir = dirs::home_dir().unwrap().join("projects");
    let symlink_path = projects_dir.join(name);

    // Determine actual project path
    let project_path =
        find_project_path(name).ok_or_else(|| anyhow!("Project '{}' not found", name))?;

    if !force {
        print!(
            "⚠️  Are you sure you want to permanently remove '{}' ? [y/N]: ",
            name
        );
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        if !matches!(input.trim().to_lowercase().as_str(), "y" | "yes") {
            println!("❎ Aborted removal of '{}'", name);
            return Ok(());
        }
    }

    // If the project is **inside ~/projects/**, just delete it
    if project_path.starts_with(&projects_dir) {
        fs::remove_dir_all(&project_path)
            .with_context(|| format!("Failed to delete project '{}'", project_path.display()))?;
    } else {
        // Project is outside ~/projects, delete actual path
        fs::remove_dir_all(&project_path)
            .with_context(|| format!("Failed to delete project '{}'", project_path.display()))?;

        // Then remove symlink in ~/projects/ if it exists
        if symlink_path.exists() {
            fs::remove_file(&symlink_path).or_else(|_| fs::remove_dir_all(&symlink_path))?;
            println!("🔗 Removed symlink '{}'", symlink_path.display());
        }
    }

    println!("🗑️  Project '{}' removed successfully", name);
    Ok(())
}

pub fn clone_project(source: &str, dest: Option<&str>, git_clone: bool) -> anyhow::Result<()> {
    use anyhow::{Context, anyhow};
    use serde_json::json;
    use walkdir::WalkDir;

    // --- Resolve destination path ---
    let dest_path: PathBuf = if let Some(d) = dest {
        let path = PathBuf::from(d);

        if path.as_os_str() == "." || path.as_os_str() == "./" {
            // Current directory: create folder with source project name
            let name = source
                .split('/')
                .last()
                .unwrap_or("cloned_project")
                .trim_end_matches(".git");
            Path::new(".").join(name)
        } else if path.is_absolute() {
            // Absolute path: always append project name
            let name = source
                .split('/')
                .last()
                .unwrap_or("cloned_project")
                .trim_end_matches(".git");
            path.join(name)
        } else {
            // Relative name inside ~/projects
            projects_dir().join(path)
        }
    } else {
        // No dest → default to ~/projects/<source_name>
        let name = source
            .split('/')
            .last()
            .unwrap_or("cloned_project")
            .trim_end_matches(".git");
        projects_dir().join(name)
    };

    if dest_path.exists() {
        anyhow::bail!("Destination '{}' already exists", dest_path.display());
    }

    fs::create_dir_all(dest_path.parent().unwrap()).with_context(|| {
        format!(
            "Failed to create parent directory '{}'",
            dest_path.display()
        )
    })?;

    // --- Determine if source is a Git URL ---
    if source.starts_with("http://") || source.starts_with("https://") || source.starts_with("git@")
    {
        println!(
            "🌐 Cloning repository '{}' into '{}'",
            source,
            dest_path.display()
        );

        let status = Command::new("git")
            .arg("clone")
            .arg(source)
            .arg(&dest_path)
            .status()
            .with_context(|| "Failed to run `git clone`")?;

        if !status.success() {
            anyhow::bail!("Git clone failed with exit code {:?}", status.code());
        }

        println!("✅ Repository cloned successfully");
    } else {
        // Local project
        let source_path = find_project_path(source)
            .ok_or_else(|| anyhow!("Source project '{}' not found", source))?;

        if git_clone && source_path.join(".git").exists() {
            println!(
                "🌱 Cloning local Git repository '{}' into '{}'",
                source_path.display(),
                dest_path.display()
            );

            let status = Command::new("git")
                .arg("clone")
                .arg(&source_path)
                .arg(&dest_path)
                .status()
                .with_context(|| "Failed to run `git clone` for local repo")?;

            if !status.success() {
                anyhow::bail!("Git clone failed with exit code {:?}", status.code());
            }
        } else {
            println!(
                "📁 Copying project '{}' into '{}'",
                source_path.display(),
                dest_path.display()
            );

            fs_extra::dir::copy(
                &source_path,
                &dest_path,
                &fs_extra::dir::CopyOptions::new().copy_inside(true),
            )
            .with_context(|| "Failed to copy project directory")?;
        }
    }

    // --- Generate .proj/project.json if missing ---
    let proj_file = dest_path.join(".proj/project.json");
    if !proj_file.exists() {
        fs::create_dir_all(proj_file.parent().unwrap())?;

        let project_name = dest_path
            .file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("cloned_project"))
            .to_string_lossy()
            .to_string();

        // Template = git URL if cloned
        let template = if source.starts_with("http://")
            || source.starts_with("https://")
            || source.starts_with("git@")
        {
            Some(source.to_string())
        } else {
            None
        };

        // Description from README
        let mut description = String::new();
        for name in &["README.md", "README.mkd", "README"] {
            let readme_path = dest_path.join(name);
            if readme_path.exists() {
                if let Ok(content) = fs::read_to_string(readme_path) {
                    description = content.lines().take(3).collect::<Vec<_>>().join(" ");
                    break;
                }
            }
        }

        let mut version = "0.0.1".to_string();

        // Try latest Git tag if git repo
        if dest_path.join(".git").exists() {
            if let Ok(output) = Command::new("git")
                .arg("describe")
                .arg("--tags")
                .arg("--abbrev=0")
                .current_dir(&dest_path)
                .output()
            {
                if output.status.success() {
                    let ver = String::from_utf8_lossy(&output.stdout);
                    version = ver.trim().to_string();
                }
            }
        }

        // Check info.py recursively
        fn find_info_py(path: &Path) -> Option<std::path::PathBuf> {
            for entry in WalkDir::new(path).into_iter().flatten() {
                if entry.file_name() == "info.py" {
                    return Some(entry.path().to_path_buf());
                }
            }
            None
        }

        if version == "0.0.1" {
            if let Some(info_path) = find_info_py(&dest_path) {
                if let Ok(content) = fs::read_to_string(&info_path) {
                    for line in content.lines() {
                        if let Some(ver) = line.strip_prefix("__version__") {
                            if let Some(ver) = ver.split('=').nth(1) {
                                version = ver
                                    .trim_matches(|c: char| {
                                        c == '\'' || c == '"' || c.is_whitespace()
                                    })
                                    .to_string();
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Check VERSION file recursively
        if version == "0.0.1" {
            for entry in WalkDir::new(&dest_path).into_iter().flatten() {
                if entry
                    .file_name()
                    .to_string_lossy()
                    .eq_ignore_ascii_case("VERSION")
                {
                    if let Ok(ver) = fs::read_to_string(entry.path()) {
                        version = ver.trim().to_string();
                        break;
                    }
                }
            }
        }

        let proj_json = json!({
            "name": project_name,
            "template": template,
            "description": description,
            "version": version,
            "completion": 1.0,
            "status": "active"
        });

        fs::write(&proj_file, serde_json::to_string_pretty(&proj_json)?)
            .with_context(|| "Failed to write project.json")?;
        println!("📦 Generated default project.json for '{}'", project_name);
    }

    // --- Link in ~/projects if outside ---
    if !dest_path.starts_with(projects_dir()) {
        link_in_projects_dir(&dest_path);
    }

    println!(
        "✅ Project '{}' cloned successfully",
        dest_path.file_name().unwrap().to_string_lossy()
    );
    Ok(())
}
