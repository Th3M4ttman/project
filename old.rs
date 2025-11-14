use clap::{Parser, Subcommand};
use serde_json::{json, Value};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::collections::HashSet;
//use fs_extra::dir::{move_dir, CopyOptions};
//use regex::Regex;
use anyhow::{anyhow, Result};
//use anyhow::Result; //{Result, anyhow};

/// Project — a simple project management and orchestration CLI tool
#[derive(Parser, Debug)]
#[command(name = "project")]
#[command(version = "0.2.2")]
#[command(about = "Automate project setup, initialization, and scanning", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
      /// initialise the current directory as a project
    Init {
        #[arg(short, long)]
        interactive: bool,
        #[arg(short, long)]
        template: Option<String>,
        #[arg(value_parser = parse_key_val::<String, String>)]
        vars: Vec<(String, String)>,
    },
        /// Create a new project
    Create {
        name: String,
        #[arg(short, long)]
        template: Option<String>,
        #[arg(value_parser = parse_key_val::<String, String>)]
        vars: Vec<(String, String)>,
        #[arg(short, long)]
        interactive: bool,
    },
        /// Scan for projects
    Scan {
        #[arg(short, long)]
        recursive: bool,
    },
        /// Set a project variable
    Set {
        #[arg(value_parser = parse_key_val::<String, String>)]
        vars: Vec<(String, String)>,
    },
        /// Get a project variable
    Get {
        key: String,
    },
        /// list all projects 
    List {
        #[arg(short, long, default_value = "active")]
        status: String,

        /// Show progress bars
        #[arg(short, long)]
        progress: bool,
    },
        /// Move a project to destination (defaults to ~/projects/<project name>/)
    Migrate {
        /// Name of the project to move
        name: String,

        /// Optional destination directory (defaults to ~/projects)
        #[arg(short, long)]
        destination: Option<PathBuf>,

        /// If set, copy instead of move
        #[arg(short, long)]
        copy: bool,
    },
        /// Remove a project
    Remove {
        /// Name of the project to remove
        name: String,

        /// Force removal without confirmation
        #[arg(short, long)]
        force: bool,
    },
        /// Clone a project from github
    Clone {
        source: String,

        dest: Option<String>,

        #[arg(short, long)]
        git_clone: bool,
    },
        /// Archive a project
    Archive {
        name: String,
        destination: Option<PathBuf>, // optional archive directory
    },
        /// List all archived projects
    Archives,

    /// Remove a specific archived project
    ArchiveRemove {
        name: String,
    },

    /// Restore an archived project
    Restore {
        name: String,
        #[arg(short, long)]
        destination: Option<String>,
    },

}

fn parse_key_val<T, U>(s: &str) -> Result<(T, U), String>
where
    T: std::str::FromStr,
    T::Err: ToString,
    U: std::str::FromStr,
    U::Err: ToString,
{
    let pos = s.find('=').ok_or_else(|| format!("invalid KEY=value: no `=` found in `{}`", s))?;
    let key: T = s[..pos].parse().map_err(|e: T::Err| e.to_string())?;
    let value: U = s[pos + 1..].parse().map_err(|e: U::Err| e.to_string())?;
    Ok((key, value))
}


fn find_project_path(name: &str) -> Option<PathBuf> {
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


fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Init { interactive, template, vars } => {
            ensure_projects_dir().unwrap();
            init_project(*interactive, template.clone(), vars);
        }
        Commands::Create { name, template, vars, interactive } => {
            ensure_projects_dir().unwrap();
            create_project(name, template.clone(), vars, *interactive);
        }
        Commands::Scan { recursive } => scan_for_proj(*recursive),
        Commands::Set { vars } => set_project_vars(vars),
        Commands::Get { key } => get_project_var(key),
        Commands::List { status, progress } => list_projects(status, *progress),
        Commands::Migrate { name, destination, copy: _ } => migrate_project(name, destination.clone()).expect("Migration failed"),
        Commands::Remove { name, force } => remove_project(name, *force).expect("Failed to remove project"),
        Commands::Clone { source, dest, git_clone } =>  clone_project(source, dest.as_deref(), *git_clone)
        .expect("Failed to clone project"),
        Commands::Archive { name, .. } => archive_project(name).expect("Failed to archive project"),
        Commands::Archives => list_archives()?,
        Commands::ArchiveRemove { name } => remove_archive(name)?,
        Commands::Restore { name, destination } => restore_archive(&name, destination.as_deref())?,
    }
  Ok(())
}

/// Return the central projects directory (`~/projects`)
fn projects_dir() -> PathBuf {
    dirs::home_dir().unwrap().join("projects")
}

/// Make sure `~/projects` exists
fn ensure_projects_dir() -> std::io::Result<()> {
    let dir = projects_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir)?;
    }
    Ok(())
}

/// Create a symlink in `~/projects` if project is outside of it
fn link_in_projects_dir(project_path: &Path) {
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

fn maybe_create_upstream(project_name: &str, project_path: &Path) {
    println!("Do you want to create a GitHub repository for '{}' and push the current branch? [y/N]: ", project_name);
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
fn init_project(interactive: bool, template: Option<String>, vars: &[(String, String)]) {
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
    let mut json_data = read_json(&proj_file);

    for (k, v) in vars {
        json_data[k] = Value::String(v.clone());
    }

    if json_data.get("template").and_then(|v| v.as_str()).is_none() {
        let chosen_template = template.or_else(select_template);
        if let Some(t) = chosen_template {
            apply_boilr_template(&t, &proj_file, interactive);
            json_data["template"] = Value::String(t);
        }
    }

    fs::write(&proj_file, serde_json::to_string_pretty(&json_data).unwrap())
        .expect("Failed to update project.json");

    // Link project in ~/projects if outside
    if !current_dir.starts_with(projects_dir()) {
        link_in_projects_dir(&current_dir);
    }
    // After applying the Boilr template
    if Command::new("git").arg("rev-parse").arg("--is-inside-work-tree")
    .current_dir(&current_dir)
    .output()
    .map(|o| o.status.success())
    .unwrap_or(false)
    {
    // Stage all files
    let _ = Command::new("git").arg("add").arg("-A")
        .current_dir(&current_dir)
        .status();

    // Commit
    let _ = Command::new("git").arg("commit").arg("-m").arg("initial commit")
        .current_dir(&current_dir)
        .status();

    // Push and set upstream
    let _ = Command::new("git").arg("push").arg("--set-upstream").arg("origin").arg("master")
        .current_dir(&current_dir)
        .status();
    }
}

/// Create a new project directory
fn create_project(
    name: &str,
    template: Option<String>,
    vars: &[(String, String)],
    interactive: bool,
) {
    let path = Path::new(name).canonicalize().unwrap_or_else(|_| Path::new(name).to_path_buf());
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

/// Apply a Boilr template
fn apply_boilr_template(template: &str, json_path: &Path, interactive: bool) {
    let json_data = fs::read_to_string(json_path).unwrap_or_else(|_| "{}".into());
    let boilr_path = "boilr";

    let mut cmd = Command::new(boilr_path);
    cmd.arg("template")
        .arg("use")
        .arg(template)
        .arg(".");

    if !interactive {
        cmd.arg("--use-defaults");
        cmd.arg("-d").arg(&json_data);
    }

    cmd.stdin(Stdio::inherit())
       .stdout(Stdio::inherit())
       .stderr(Stdio::inherit());

    println!("⚙️ Applying boilr template: {}", template);

    let status = cmd.status().expect("Failed to run boilr");

    if !status.success() {
        eprintln!("❌ Boilr failed with exit code {}", status);
    }
}

/// Ask the user to choose a template interactively
fn select_template() -> Option<String> {
    let home_dir = env::var("HOME").ok()?;
    let templates_dir = Path::new(&home_dir).join(".config/boilr/templates");
    let entries = fs::read_dir(&templates_dir).ok()?;

    let templates: Vec<String> = entries
        .flatten()
        .filter_map(|e| {
            if e.path().is_dir() {
                e.file_name().to_str().map(|s| s.to_string())
            } else {
                None
            }
        })
        .collect();

    if templates.is_empty() {
        eprintln!("No templates found in {}", templates_dir.display());
        return None;
    }

    println!("Available templates:");
    for (i, t) in templates.iter().enumerate() {
        println!("  {}. {}", i + 1, t);
    }

    print!("Select template: ");
    io::stdout().flush().unwrap();

    let mut input = String::new();
    io::stdin().read_line(&mut input).ok()?;
    let trimmed = input.trim();

    if let Ok(index) = trimmed.parse::<usize>() {
        if index > 0 && index <= templates.len() {
            return Some(templates[index - 1].clone());
        }
    }

    templates.iter()
        .find(|t| t.eq_ignore_ascii_case(trimmed))
        .cloned()
}

fn read_json(path: &Path) -> Value {
    if let Ok(content) = fs::read_to_string(path) {
        serde_json::from_str(&content).unwrap_or(json!({}))
    } else {
        json!({})
    }
}

fn set_project_vars(vars: &[(String, String)]) {
    let proj_file = Path::new(".proj/project.json");
    let mut data = read_json(proj_file);

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

fn get_project_var(key: &str) {
    let proj_file = Path::new(".proj/project.json");
    let data = read_json(proj_file);

    match data.get(key) {
        Some(val) => println!("{}", val),
        None => eprintln!("Key '{}' not found.", key),
    }
}

fn init_git_repo(path: &Path) {
    if path.join(".git").exists() {
        return;
    }
    let _ = Command::new("git").arg("init").current_dir(path).output();
    
}



fn scan_for_proj(recursive: bool) {
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
                                path.file_name()
                                    .unwrap_or_default()
                                    .to_string_lossy()
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

fn git_status_flags(path: &Path) -> (bool, bool, bool) {
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
        ||
        Command::new("git")
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

fn list_projects(status_filter: &str, show_progress: bool) {
    ensure_projects_dir().ok();

    let mut seen = std::collections::HashSet::new();

    /// Recursively scan directories for projects
    fn visit(dir: &Path, recursive: bool, seen: &mut std::collections::HashSet<PathBuf>) -> Vec<PathBuf> {
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

        let data = read_json(&proj_file);

        let status = data.get("status").and_then(|v| v.as_str()).unwrap_or("active");
        let completion = data.get("completion").and_then(|v| v.as_f64()).unwrap_or(0.0);

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
        if unadded { flags.push_str("\x1b[31m+\x1b[0m"); }
        if uncommitted { flags.push_str("\x1b[31mc\x1b[0m"); }
        if unpushed { flags.push_str("\x1b[31m^\x1b[0m"); }

        let project_name = project_path.file_name().unwrap_or_default().to_string_lossy();

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
                "{}{}{}{}",
                color,
                "█".repeat(filled),
                "\x1b[0m",
                "░".repeat(empty)
            );

            println!("{} {} [{}] {:.0}%", project_name, flags, bar, completion * 100.0);
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





fn migrate_project(name: &str, destination: Option<PathBuf>) -> Result<()> {
    let default_dest = dirs::home_dir().unwrap().join("projects");
    let destination = destination.unwrap_or(default_dest);
    let dest_path = destination.join(name);

    // First try the registered project path
    let project_path = find_project_path(name)
        // fallback: check if a directory exists in cwd or home
        .or_else(|| {
            let cwd_path = std::env::current_dir().ok()?.join(name);
            if cwd_path.exists() { Some(cwd_path) } else { None }
        })
        .ok_or_else(|| anyhow::anyhow!("Project '{}' not found", name))?;

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

    println!("✅ Project '{}' migrated to '{}'", name, dest_path.display());
    Ok(())
}

fn remove_project(name: &str, force: bool) -> anyhow::Result<()> {
    use std::fs;
    use std::io::{self, Write};
    use anyhow::{anyhow, Context};
    

    let projects_dir = dirs::home_dir().unwrap().join("projects");
    let symlink_path = projects_dir.join(name);

    // Determine actual project path
    let project_path = find_project_path(name)
        .ok_or_else(|| anyhow!("Project '{}' not found", name))?;

    if !force {
        print!("⚠️  Are you sure you want to permanently remove '{}' ? [y/N]: ", name);
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

fn clone_project(
    source: &str,
    dest: Option<&str>,
    git_clone: bool,
) -> anyhow::Result<()> {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use anyhow::{anyhow, Context};
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

    fs::create_dir_all(dest_path.parent().unwrap())
        .with_context(|| format!("Failed to create parent directory '{}'", dest_path.display()))?;

    // --- Determine if source is a Git URL ---
    if source.starts_with("http://") || source.starts_with("https://") || source.starts_with("git@") {
        println!("🌐 Cloning repository '{}' into '{}'", source, dest_path.display());

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
            println!("🌱 Cloning local Git repository '{}' into '{}'", source_path.display(), dest_path.display());

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
            println!("📁 Copying project '{}' into '{}'", source_path.display(), dest_path.display());

            fs_extra::dir::copy(
                &source_path,
                &dest_path,
                &fs_extra::dir::CopyOptions::new().copy_inside(true),
            ).with_context(|| "Failed to copy project directory")?;
        }
    }

    // --- Generate .proj/project.json if missing ---
    let proj_file = dest_path.join(".proj/project.json");
    if !proj_file.exists() {
        fs::create_dir_all(proj_file.parent().unwrap())?;

        let project_name = dest_path.file_name()
            .unwrap_or_else(|| std::ffi::OsStr::new("cloned_project"))
            .to_string_lossy()
            .to_string();

        // Template = git URL if cloned
        let template = if source.starts_with("http://") || source.starts_with("https://") || source.starts_with("git@") {
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
                                    .trim_matches(|c: char| c == '\'' || c == '"' || c.is_whitespace())
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
                if entry.file_name().to_string_lossy().eq_ignore_ascii_case("VERSION") {
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

    println!("✅ Project '{}' cloned successfully", dest_path.file_name().unwrap().to_string_lossy());
    Ok(())
}

fn archive_project(project_name: &str) -> Result<()> {
    
    use anyhow::{anyhow, Context};
    use std::fs;
    
    use std::io::{Write, Read};
    use chrono::Local;

    let projects_dir = dirs::home_dir()
        .ok_or_else(|| anyhow!("Could not locate home directory"))?
        .join(".proj/projects");
    let project_dir = projects_dir.join(project_name);

    // 🧩 If project.json doesn’t exist here, check ~/projects/<name>
    let real_path = if project_dir.exists() {
        project_dir
    } else {
        let alt_path = dirs::home_dir()
            .ok_or_else(|| anyhow!("Could not locate home directory"))?
            .join("projects")
            .join(project_name);

        if alt_path.exists() {
            alt_path
        } else {
            // Fallback: maybe it’s in current working directory
            let cwd_path = std::env::current_dir()?.join(project_name);
            if cwd_path.exists() {
                cwd_path
            } else {
                return Err(anyhow!("Project '{}' not found", project_name));
            }
        }
    };

    // 📦 Prepare archive directory
    let archive_dir = dirs::home_dir()
        .ok_or_else(|| anyhow!("Could not locate home directory"))?
        .join(".proj/archives");
    fs::create_dir_all(&archive_dir)?;

    let timestamp = Local::now().format("%Y%m%d_%H%M%S").to_string();
    let archive_path = archive_dir.join(format!("{}_{}.zip", project_name, timestamp));

    let zip_file = std::fs::File::create(&archive_path)
        .with_context(|| format!("Could not create archive file: {}", archive_path.display()))?;

    let mut zip = zip::ZipWriter::new(zip_file);
    let options: zip::write::FileOptions<'_, ()> =
        zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    // 🧾 Recursively add files
    for entry in walkdir::WalkDir::new(&real_path) {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() {
            let name_in_zip = path.strip_prefix(&real_path).unwrap().to_str().unwrap();
            zip.start_file(name_in_zip, options)?;
            let mut f = std::fs::File::open(path)?;
            let mut buffer = Vec::new();
            f.read_to_end(&mut buffer)?;
            zip.write_all(&buffer)?;
        }
    }

    zip.finish()?;

    println!(
        "📦 Archived project '{}' to {}",
        project_name,
        archive_path.display()
    );

    // 🗑️ Remove project directory and symlink after archiving
    if real_path.exists() {
        std::fs::remove_dir_all(&real_path)
            .with_context(|| format!("Failed to delete {}", real_path.display()))?;
    }

    let projects_link = dirs::home_dir().unwrap().join("projects").join(project_name);
    if projects_link.exists() {
        std::fs::remove_file(&projects_link).ok();
    }

    Ok(())
}

fn get_archives_dir() -> PathBuf {
    dirs::home_dir().unwrap().join(".proj/archives")
}

fn list_archives() -> Result<()> {
    use std::fs;
    let archives_dir = get_archives_dir();

    if !archives_dir.exists() {
        println!("No archives found.");
        return Ok(());
    }

    let entries = fs::read_dir(&archives_dir)?;
    let mut found_any = false;

    for entry in entries {
        let entry = entry?;
        if entry.path().extension().map(|e| e == "zip").unwrap_or(false) {
            let file_name = entry.file_name().into_string().unwrap_or_default();
            println!("📦 {}", file_name.trim_end_matches(".zip"));
            found_any = true;
        }
    }

    if !found_any {
        println!("No archives found.");
    }

    Ok(())
}

fn remove_archive(name: &str) -> Result<()> {
    use std::fs;
    let archives_dir = get_archives_dir();
    let archive_path = archives_dir.join(format!("{}.zip", name));

    if !archive_path.exists() {
        return Err(anyhow!("Archive '{}' not found", name));
    }

    fs::remove_file(&archive_path)?;
    println!("🗑️  Removed archive '{}'", name);
    Ok(())
}

fn restore_archive(archive_name: &str, destination: Option<&str>) -> Result<()> {
    use std::fs;
    use std::io;
    use std::os::unix::fs as unix_fs;
    use std::path::{Path, PathBuf};
    use zip::ZipArchive;
    use std::fs::File;
    use anyhow::{anyhow, Result};

    let archives_dir = get_archives_dir();
    let archive_path = archives_dir.join(format!("{}.zip", archive_name));

    if !archive_path.exists() {
        return Err(anyhow!("Archive '{}' not found", archive_name));
    }

    // Extract original project name from archive
    // This assumes archives are named like "projectname_YYYYMMDD_HHMMSS.zip"
    let original_name = archive_name
        .splitn(2, '_')
        .next()
        .ok_or_else(|| anyhow!("Failed to parse original project name from '{}'", archive_name))?;

    // Determine destination folder
    let dest_path = if let Some(dest) = destination {
        PathBuf::from(dest).join(original_name)
    } else {
        dirs::home_dir()
            .ok_or_else(|| anyhow!("Failed to locate home directory"))?
            .join("projects")
            .join(original_name)
    };

    if dest_path.exists() {
        return Err(anyhow!(
            "Destination folder '{}' already exists",
            dest_path.display()
        ));
    }

    fs::create_dir_all(&dest_path)?;
    let file = File::open(&archive_path)?;
    let mut zip = ZipArchive::new(file)?;

    for i in 0..zip.len() {
        let mut file = zip.by_index(i)?;
        let outpath = dest_path.join(file.mangled_name());

        if file.name().ends_with('/') {
            fs::create_dir_all(&outpath)?;
        } else {
            if let Some(p) = outpath.parent() {
                fs::create_dir_all(p)?;
            }
            let mut outfile = fs::File::create(&outpath)?;
            io::copy(&mut file, &mut outfile)?;
        }
    }

    // Create symlink in ~/projects if restoring outside of projects
    let projects_dir = dirs::home_dir()
        .ok_or_else(|| anyhow!("Failed to locate home directory"))?
        .join("projects");
    if !dest_path.starts_with(&projects_dir) {
        let symlink_path = projects_dir.join(original_name);
        if symlink_path.exists() {
            fs::remove_file(&symlink_path)?;
        }
        unix_fs::symlink(&dest_path, &symlink_path)?;
        println!(
            "🔗 Created symlink from '{}' → '{}'",
            symlink_path.display(),
            dest_path.display()
        );
    }

    println!("✅ Restored archive '{}' to '{}'", archive_name, dest_path.display());
    Ok(())
}

