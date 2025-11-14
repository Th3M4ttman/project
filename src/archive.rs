use anyhow::{anyhow, Context, Result};
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use chrono::Local;
use zip::ZipArchive;
use std::fs::File;

/// Return the archives directory (`~/.proj/archives`)
pub fn get_archives_dir() -> PathBuf {
    dirs::home_dir().unwrap().join(".proj/archives")
}

pub fn archive_project(project_name: &str) -> Result<()> {
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
    let archive_dir = get_archives_dir();
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

pub fn list_archives() -> Result<()> {
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

pub fn remove_archive(name: &str) -> Result<()> {
    let archives_dir = get_archives_dir();
    let archive_path = archives_dir.join(format!("{}.zip", name));

    if !archive_path.exists() {
        return Err(anyhow!("Archive '{}' not found", name));
    }

    fs::remove_file(&archive_path)?;
    println!("🗑️  Removed archive '{}'", name);
    Ok(())
}

pub fn restore_archive(archive_name: &str, destination: Option<&str>) -> Result<()> {
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
        std::os::unix::fs::symlink(&dest_path, &symlink_path)?;
        println!(
            "🔗 Created symlink from '{}' → '{}'",
            symlink_path.display(),
            dest_path.display()
        );
    }

    println!("✅ Restored archive '{}' to '{}'", archive_name, dest_path.display());
    Ok(())
}
