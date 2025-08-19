use clap::{Parser, Subcommand};
use std::{
    fs::{File, self},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use sha1::{Digest, Sha1};
use chrono::Utc;
use flate2::{
    read::GzDecoder,
    write::GzEncoder,
    Compression,
};
use diffy::{create_patch, Patch, apply};

const RIF_DIR: &str = ".rif";

#[derive(Parser)]
#[command(name = "rif")]
#[command(about = "Minimal, space-efficient version control for Ableton!")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
    Commit {
        #[arg(short, long)]
        message: String,
    },
    Checkout {
        #[arg(long)]
        hash: String,
    },
    Log,
    Open,
}

struct Commit {
    parent_hash: Option<String>,
    message: String,
    timestamp: i64,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init => init_repo()?,
        Commands::Commit { message } => commit(&message)?,
        Commands::Checkout { hash } => checkout(&hash)?,
        Commands::Log => log()?,
        Commands::Open => open_in_ableton()?,
    }

    Ok(())
}

fn init_repo() -> anyhow::Result<()> {
    let rif_dir = Path::new(RIF_DIR);

    if rif_dir.exists() {
        println!("rif repository already exists.");
        return Ok(());
    }

    fs::create_dir(rif_dir)?;
    fs::create_dir(rif_dir.join("objects"))?;
    fs::create_dir_all(rif_dir.join("refs/heads"))?;

    fs::write(rif_dir.join("HEAD"), "ref: refs/heads/main\n")?;

    println!("Initialized empty rif repository in .rif");
    Ok(())
}

fn commit(message: &str) -> anyhow::Result<()> {
    let rif_dir = Path::new(RIF_DIR);

    if !rif_dir.exists() {
        anyhow::bail!("Not a rif repository. Run `rif init` first.");
    }

    let (current_commit, snapshot) = create_commit(rif_dir, message)?;
    
    if let Some(parent_hash) = &current_commit.parent_hash {
        update_parent_to_diff(rif_dir, parent_hash, &snapshot)?;
    }

    let commit_hash = save_commit(rif_dir, &current_commit, &snapshot)?;
    
    let branch_path = rif_dir.join("refs/heads/main");
    fs::write(branch_path, &commit_hash)?;

    let short_hash = &commit_hash[0..6];
    println!("[main {}] {}", short_hash, message);

    Ok(())
}    

fn create_commit(rif_dir: &Path, message: &str) -> anyhow::Result<(Commit, String)> {
    let als_path = find_als_file()?;
    let current_snapshot = decompress_als(&als_path)?;

    let head_hash = get_head_hash(rif_dir)?;
    
    let commit = Commit {
        parent_hash: head_hash,
        message: message.to_string(),
        timestamp: Utc::now().timestamp(),
    };
    
    Ok((commit, current_snapshot))
}

fn save_commit(rif_dir: &Path, commit: &Commit, snapshot: &str) -> anyhow::Result<String> {
    let commit_str = serialize_commit(commit);
    let commit_hash = hash_content(&commit_str);
    let commit_path = rif_dir.join("objects").join(&commit_hash);

    if commit_path.exists() {
        anyhow::bail!("Commit with hash {} already exists", commit_hash);
    }

    // Save the commit metadata
    fs::write(commit_path, commit_str)?;
    
    // Save the full snapshot for this commit
    save_snapshot(rif_dir, &commit_hash, snapshot)?;

    Ok(commit_hash)
}

fn serialize_commit(commit: &Commit) -> String {
    let mut out = String::new();

    if let Some(ref parent) = commit.parent_hash {
        out.push_str(&format!("parent {}\n", parent));
    }
    out.push_str(&format!("message {}\n", commit.message.replace('\n', " ")));
    out.push_str(&format!("timestamp {}\n", commit.timestamp));

    out
}

fn apply_diff(current_content: &str, diff_content: &str) -> anyhow::Result<String> {
    let patch = Patch::from_str(diff_content)?;
    Ok(apply(current_content, &patch)?)
}

fn parse_commit(data: &str) -> anyhow::Result<Commit> {
    let mut parent = None;
    let mut message = String::new();
    let mut timestamp = 0;

    for line in data.lines() {
        if let Some(rest) = line.strip_prefix("parent ") {
            parent = Some(rest.to_string());
        } else if let Some(rest) = line.strip_prefix("message ") {
            message = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("timestamp ") {
            timestamp = rest.parse()?;
        }
    }

    Ok(Commit {
        parent_hash: parent,
        message,
        timestamp,
    })
}

fn get_head_hash(rif_dir: &Path) -> anyhow::Result<Option<String>> {
    let current_branch = get_current_branch(rif_dir)?;
    let branch_path = rif_dir.join(&current_branch);

    if !branch_path.exists() {
        return Ok(None);
    }

    let head_hash = fs::read_to_string(&branch_path)?.trim().to_string();

    if head_hash.is_empty() {
        anyhow::bail!("Branch {} head is empty", current_branch);
    }

    Ok(Some(head_hash))
}

fn hash_content(content: &str) -> String {
    let mut hasher = Sha1::new();
    hasher.update(content.as_bytes());
    let hash = hasher.finalize();

    format!("{:x}", hash)
}

fn get_current_branch(rif_dir: &Path) -> anyhow::Result<String> {
    let head_path = rif_dir.join("HEAD");

    if !head_path.exists() {
        anyhow::bail!("HEAD file does not exist. Is this a rif repository?");
    }

    let head_data = fs::read_to_string(&head_path)?.trim().to_string();

    if !head_data.starts_with("ref: refs/heads/") {
        anyhow::bail!("HEAD file does not point to a valid branch reference");
    }

    let branch_name = head_data.trim_start_matches("ref: ").to_string();
    Ok(branch_name)
}

fn find_als_file() -> anyhow::Result<PathBuf> {
    let dir = std::env::current_dir()?;

    for entry in fs::read_dir(&dir)? {
        let entry = entry?;
        
        let path = entry.path();
        if let Some(ext) = path.extension() {
            if ext == "als" {
                return Ok(path);
            }
        }
    }

    anyhow::bail!("No .als file found in current directory");
}

fn decompress_als(path: &Path) -> anyhow::Result<String> {
    let file = fs::File::open(path)?;
    let mut decoder = GzDecoder::new(file);
    let mut s = String::new();
    decoder.read_to_string(&mut s)?;
    Ok(s)
}

fn checkout(hash: &str) -> anyhow::Result<()> {
    let rif_dir = Path::new(RIF_DIR);

    let content = construct_snapshot(rif_dir, hash)?;
    
    let als_path = find_als_file()?;
    let file = File::create(&als_path)?;
    let mut encoder = GzEncoder::new(file, Compression::default());
    encoder.write_all(content.as_bytes())?;
    encoder.finish()?;

    println!("Checked out commit {} to {:?}", hash, als_path);

    Ok(())
}

fn get_snapshot(rif_dir: &Path, commit_hash: &str) -> anyhow::Result<String> {
    let snapshot_path = rif_dir.join("objects").join(format!("{}.snapshot.gz", commit_hash));
    if !snapshot_path.exists() {
        anyhow::bail!("Snapshot not found for commit {}", commit_hash)
    }
    
    let compressed_data = fs::read(&snapshot_path)?;
    decompress_content(&compressed_data)
}

fn save_snapshot(rif_dir: &Path, commit_hash: &str, content: &str) -> anyhow::Result<()> {
    let snapshot_path = rif_dir.join("objects").join(format!("{}.snapshot.gz", commit_hash));
    let compressed_data = compress_content(content)?;
    fs::write(snapshot_path, compressed_data)?;
    Ok(())
}

fn delete_snapshot(rif_dir: &Path, commit_hash: &str) -> anyhow::Result<()> {
    let snapshot_path = rif_dir.join("objects").join(format!("{}.snapshot.gz", commit_hash));
    if snapshot_path.exists() {
        fs::remove_file(snapshot_path)?;
    }
    Ok(())
}

fn get_diff(rif_dir: &Path, commit_hash: &str) -> anyhow::Result<String> {
    let diff_path = rif_dir.join("objects").join(format!("{}.diff.gz", commit_hash));
    if !diff_path.exists() {
        anyhow::bail!("Diff not found for commit {}", commit_hash)
    }
    
    let compressed_data = fs::read(&diff_path)?;
    decompress_content(&compressed_data)
}

fn save_diff(rif_dir: &Path, commit_hash: &str, diff: &str) -> anyhow::Result<()> {
    let diff_path = rif_dir.join("objects").join(format!("{}.diff.gz", commit_hash));
    let compressed_data = compress_content(diff)?;
    fs::write(diff_path, compressed_data)?;
    Ok(())
}

fn update_parent_to_diff(rif_dir: &Path, parent_hash: &str, current_snapshot: &str) -> anyhow::Result<()> {
    let parent_snapshot = get_snapshot(rif_dir, parent_hash)?;
    
    if parent_snapshot == current_snapshot {
        anyhow::bail!("No changes detected between this commit and parent. Cannot create an empty commit.");
    }

    let patch = create_patch(current_snapshot, &parent_snapshot);
    let parent_diff = patch.to_string();
    
    save_diff(rif_dir, parent_hash, &parent_diff)?;
    
    delete_snapshot(rif_dir, parent_hash)?;
    
    Ok(())
}

fn construct_snapshot(rif_dir: &Path, target_hash: &str) -> anyhow::Result<String> {
    let mut current_hash = match get_head_hash(rif_dir)? {
        Some(hash) => hash,
        None => anyhow::bail!("Cannot construct snapshot: no HEAD commit found"),
    };

    // we know there is a snapshot from the head hash
    let mut content = get_snapshot(rif_dir, &current_hash)?;

    while current_hash[0..6] != target_hash[0..6] {
        let commit_path = rif_dir.join("objects").join(&current_hash);
        let commit_data = fs::read_to_string(&commit_path)?;
        let commit = parse_commit(&commit_data)?;
        
        if let Some(parent_hash) = commit.parent_hash {
            let diff_content = get_diff(rif_dir, &parent_hash)?;
            content = apply_diff(&content, &diff_content)?;
            
            current_hash = parent_hash;
        } else {
            anyhow::bail!("Commit not found: reached root without finding target {}", target_hash);
        }
    }
    
    Ok(content)
}

fn log() -> anyhow::Result<()> {
    let rif_dir = Path::new(RIF_DIR);

    if !rif_dir.exists() {
        anyhow::bail!("Not a rif repository. Run `rif init` first.");
    }

    let head_hash = match get_head_hash(rif_dir)? {
        Some(hash) => hash,
        None => {
            println!("No commits yet.");
            return Ok(());
        }
    };

    let mut current_hash = head_hash;

    loop {
        let commit_path = rif_dir.join("objects").join(&current_hash);
        if !commit_path.exists() {
            break;
        }

        let commit_data = fs::read_to_string(&commit_path)?;
        let commit = parse_commit(&commit_data)?;
        
        let short_hash = &current_hash[0..6];
        
        println!("{} {}", short_hash, commit.message);
        
        match commit.parent_hash {
            Some(parent) => current_hash = parent,
            None => break,
        }
    }

    Ok(())
}

fn compress_content(content: &str) -> anyhow::Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(content.as_bytes())?;
    encoder.finish().map_err(|e| anyhow::anyhow!(e))
}

fn decompress_content(compressed_data: &[u8]) -> anyhow::Result<String> {
    let mut decoder = GzDecoder::new(compressed_data);
    let mut content = String::new();
    decoder.read_to_string(&mut content)?;
    Ok(content)
}

fn open_in_ableton() -> anyhow::Result<()> {
    let als_path = find_als_file()?;

    let apps = fs::read_dir("/Applications")?
        .filter_map(Result::ok)
        .map(|e| e.file_name().into_string().ok())
        .flatten()
        .filter(|name| name.starts_with("Ableton Live") && name.ends_with(".app"))
        .collect::<Vec<_>>();

    if apps.is_empty() {
        anyhow::bail!("No Ableton Live installation found in /Applications");
    }

    let mut apps_sorted = apps;
    apps_sorted.sort(); 
    let ableton_app = &apps_sorted[0];
    let app_name = ableton_app.trim_end_matches(".app");

    let status = std::process::Command::new("open")
        .arg("-a")
        .arg(app_name)
        .arg(als_path.as_os_str())
        .status()?;

    if !status.success() {
        anyhow::bail!("Failed to launch Ableton with '{}'", app_name);
    }

    println!("Opened project in '{}'", app_name);
    Ok(())
}