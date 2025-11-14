mod climod;
mod project;
mod archive;
mod template;
mod utils;

use clap::Parser;
use anyhow::Result;

/// Project — a simple project management and orchestration CLI tool
#[derive(Parser, Debug)]
#[command(name = "project")]
#[command(version = "0.2.2")]
#[command(about = "Automate project setup, initialization, and scanning", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: climod::Commands,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        climod::Commands::Init { interactive, template, vars } => {
            project::ensure_projects_dir().unwrap();
            project::init_project(*interactive, template.clone(), vars);
        }
        climod::Commands::Create { name, template, vars, interactive } => {
            project::ensure_projects_dir().unwrap();
            project::create_project(name, template.clone(), vars, *interactive);
        }
        climod::Commands::Scan { recursive } => project::scan_for_proj(*recursive),
        climod::Commands::Set { vars } => project::set_project_vars(vars),
        climod::Commands::Get { key } => project::get_project_var(key),
        climod::Commands::List { status, progress } => project::list_projects(status, *progress),
        climod::Commands::Migrate { name, destination, copy: _ } => project::migrate_project(name, destination.clone()).expect("Migration failed"),
        climod::Commands::Remove { name, force } => project::remove_project(name, *force).expect("Failed to remove project"),
        climod::Commands::Clone { source, dest, git_clone } =>  project::clone_project(source, dest.as_deref(), *git_clone)
        .expect("Failed to clone project"),
        climod::Commands::Archive { name, .. } => archive::archive_project(name).expect("Failed to archive project"),
        climod::Commands::Archives => archive::list_archives()?,
        climod::Commands::ArchiveRemove { name } => archive::remove_archive(name)?,
        climod::Commands::Restore { name, destination } => archive::restore_archive(&name, destination.as_deref())?,
    }
  Ok(())
}
