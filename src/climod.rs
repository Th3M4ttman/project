
use clap::{Args, Subcommand};
use std::path::{PathBuf};



#[derive(Args, Debug)]
pub struct TodoArgs {
    #[command(subcommand)]
    pub action: Option<TodoAction>,

    /// List todos (shortcut)
    #[arg(short = 'l', long = "list", conflicts_with_all = ["add", "remove"])]
    pub list_flag: bool,

    /// Add a todo (shortcut)
    #[arg(short = 'a', long = "add", conflicts_with_all = ["list_flag", "remove"])]
    pub add: Option<String>,

    /// Remove a todo (shortcut)
    #[arg(short = 'r', long = "remove", conflicts_with_all = ["list_flag", "add"])]
    pub remove: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum TodoAction {
    /// List todos
    #[command(alias = "l")]
    List,

    /// Add a todo
    #[command(alias = "a")]
    Add {
        text: String,
    },

    /// Remove a todo by index or text
    #[command(alias = "r")]
    Remove {
        pattern: String,
    },
}


#[derive(Subcommand, Debug)]
pub enum Commands {
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

    Todo(TodoArgs),
    
    Initshell,
}

pub fn parse_key_val<T, U>(s: &str) -> Result<(T, U), String>
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
