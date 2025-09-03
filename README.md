# rif - Minimal Version Control for Ableton Live Projects

`rif` is a lightweight, space-efficient version control system specifically designed for Ableton Live projects (.als files). 

It helps you track changes to your music projects without consuming excessive disk space, with some versions being <10% of the original file size!

## Features

- Track changes to Ableton Live project files
- Efficiently store versions using full snapshots and diffs
- Simple command-line interface
- Open projects in Ableton Live from the command line

## How It Works

`rif` uses a modified Git-like approach to version control:

1. The latest commit is stored as a full snapshot of your Ableton Live project
2. Previous versions are stored as space-efficient reverse diffs (different from git)
3. When checking out older versions, diffs are applied in reverse-chronological order to reconstruct the project state

## Installation

### Prerequisites

- Rust and Cargo installed on your system
- Ableton Live installed (if you plan to use the `open` command)

### Building from Source

```sh
git clone https://github.com/aaronthechen/rif.git
cd rif
cargo build --release
```

The binary will be available at `target/release/rif`

### Adding to PATH

To run `rif` from anywhere in your terminal, you need to add it to your system's PATH:

#### macOS/Linux

Option 1: Copy to a directory already in your PATH:
```sh
sudo cp target/release/rif /usr/local/bin/
```

Option 2: Add to your shell profile:
```sh
echo 'export PATH="$PATH:/path/to/rif/target/release"' >> ~/.zshrc
# or for bash
# echo 'export PATH="$PATH:/path/to/rif/target/release"' >> ~/.bashrc
source ~/.zshrc  # or ~/.bashrc
```

#### Windows

Option 1: Add to PATH via GUI:
1. Right-click on "This PC" or "My Computer" and select "Properties"
2. Click on "Advanced system settings"
3. Click on "Environment Variables"
4. Under "System variables" find "Path" and click "Edit"
5. Click "New" and add the full path to the rif release directory

Option 2: Using Command Prompt with admin privileges:
```cmd
setx /M PATH "%PATH%;C:\path\to\rif\target\release"
```

## Usage

### Initialize a Repository

Navigate to the directory containing your Ableton Live project (.als file) and initialize a rif repository:

```sh
rif init
```

This creates a `.rif` directory to store version information, with a similar structure to git.

### Commit Changes

After making changes to your Ableton Live project, save them in rif:

```sh
rif commit -m "Your commit message"
```

### View Commit History

To see the history of commits:

```sh
rif log
```

### Restore Previous Versions

To checkout a previous version of your project:

```sh
rif checkout --hash <commit_hash>
```

You only need to provide the first 6 characters of the commit hash.

### Open Project in Ableton Live

To open the current project in Ableton Live:

```sh
rif open
```

This command automatically detects your Ableton Live installation and opens the project file.

## Future Work

- Cross platform support for rif open
- More cli commands for Ableton
- Git integration
- Remote storage integration
- Custom compression
- Potential(?) ability to analyze and modify specific portions of Ableton files (TBD)

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## License

This project is licensed under the MIT License - see the [LICENSE-MIT](LICENSE-MIT) file for details.
