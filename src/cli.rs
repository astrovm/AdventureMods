use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand};
use console::Style;
use tokio::runtime::Builder;

use crate::banner;
use crate::config;
use crate::setup::{common, pipeline, sadx};
use crate::steam::game::{Game, GameKind};
use crate::steam::library::{self, DetectionResult};

const DEFAULT_WIDTH: u32 = 1920;
const DEFAULT_HEIGHT: u32 = 1080;

struct CliOutput<'a> {
    writer: &'a mut dyn Write,
    use_color: bool,
    bold: Style,
    cyan: Style,
    green: Style,
    dim: Style,
}

impl<'a> CliOutput<'a> {
    fn new(writer: &'a mut dyn Write, use_color: bool) -> Self {
        CliOutput {
            writer,
            use_color,
            bold: Style::new().bold(),
            cyan: Style::new().cyan().bright(),
            green: Style::new().green().bright(),
            dim: Style::new().dim(),
        }
    }

    fn heading(&mut self, s: &str) -> std::io::Result<()> {
        if self.use_color {
            writeln!(self.writer, "{}", self.bold.apply_to(s))
        } else {
            writeln!(self.writer, "{s}")
        }
    }

    fn success(&mut self, s: &str) -> std::io::Result<()> {
        if self.use_color {
            writeln!(self.writer, "{}", self.green.apply_to(s))
        } else {
            writeln!(self.writer, "{s}")
        }
    }

    fn path(&self, s: &str) -> String {
        if self.use_color {
            self.cyan.apply_to(s).to_string()
        } else {
            s.to_string()
        }
    }

    fn bold_item(&mut self, name: &str, desc: &str) -> std::io::Result<()> {
        if self.use_color {
            writeln!(
                self.writer,
                "- {}: {}",
                self.bold.apply_to(name),
                self.dim.apply_to(desc)
            )
        } else {
            writeln!(self.writer, "- {name}: {desc}")
        }
    }

    fn prompt(&mut self, s: &str) -> std::io::Result<()> {
        if self.use_color {
            write!(self.writer, "{} ", self.bold.apply_to(s))
        } else {
            write!(self.writer, "{s} ")
        }
    }

    fn writeln(&mut self, s: &str) -> std::io::Result<()> {
        writeln!(self.writer, "{s}")
    }
}

#[derive(Debug, Parser)]
#[command(name = "adventure-mods")]
pub struct Cli {
    #[arg(long)]
    no_color: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Detect(DetectArgs),
    ListMods {
        #[arg(long)]
        game: String,
    },
    Setup(SetupArgs),
}

#[derive(Debug, Args, Default)]
pub struct DetectArgs {
    #[arg(long)]
    pub libraryfolders_vdf: Option<PathBuf>,
    #[arg(long = "steam-library")]
    pub steam_libraries: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SetupArgs {
    #[arg(long)]
    pub game: Option<String>,
    #[arg(long = "mod")]
    pub mods: Vec<String>,
    #[arg(long)]
    pub preset: Option<String>,
    #[arg(long)]
    pub width: Option<u32>,
    #[arg(long)]
    pub height: Option<u32>,
    #[arg(long)]
    pub game_path: Option<PathBuf>,
    #[command(flatten)]
    pub detect: DetectArgs,
}

pub fn run_with_io(
    cli: Cli,
    use_color: bool,
    input: &mut impl Read,
    output: &mut impl Write,
) -> Result<()> {
    let mut input = BufReader::new(input);
    let mut out = CliOutput::new(output, use_color);

    match cli.command {
        Some(Command::Detect(args)) => run_detect(args, &mut out),
        Some(Command::ListMods { game }) => run_list_mods(&game, &mut out),
        Some(Command::Setup(args)) => run_setup(args, &mut input, &mut out),
        None => Ok(()),
    }
}

pub fn initialize_crypto_provider() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow!("failed to install TLS crypto provider: already installed"))?;
    Ok(())
}

fn run_detect(args: DetectArgs, out: &mut CliOutput) -> Result<()> {
    banner::print_header(&mut out.writer, config::VERSION, out.use_color)?;

    let result = detect_games_strict(&args)?;

    if result.games.is_empty() && result.inaccessible.is_empty() {
        out.writeln("No supported games detected.")?;
        return Ok(());
    }

    if !result.games.is_empty() {
        out.heading("Detected games:")?;
        for game in &result.games {
            out.writeln(&format!(
                "- {}: {}",
                game.kind.name(),
                out.path(&game.path.display().to_string())
            ))?;
        }
    }

    if !result.inaccessible.is_empty() {
        out.heading("Inaccessible Steam libraries:")?;
        for game in &result.inaccessible {
            out.writeln(&format!(
                "- {}: {}",
                game.kind.name(),
                out.path(&game.library_path.display().to_string())
            ))?;
        }
    }

    Ok(())
}

fn run_list_mods(game: &str, out: &mut CliOutput) -> Result<()> {
    let game_kind = parse_game_kind(game)?;
    banner::print_header(&mut out.writer, config::VERSION, out.use_color)?;
    out.writeln(&format!("Game: {}", game_kind.name()))?;

    let presets = common::presets_for_game(game_kind);
    if !presets.is_empty() {
        out.heading("Presets:")?;
        for preset in presets {
            out.bold_item(preset.name, preset.description)?;
        }
    }

    out.heading("Mods:")?;
    for mod_entry in common::recommended_mods_for_game(game_kind) {
        out.bold_item(mod_entry.name, mod_entry.description)?;
    }

    Ok(())
}

fn run_setup(args: SetupArgs, input: &mut impl BufRead, out: &mut CliOutput) -> Result<()> {
    let term_width = console::Term::stdout().size().1 as usize;
    if term_width >= 72 {
        banner::print_banner(&mut out.writer, out.use_color)?;
    }
    banner::print_header(&mut out.writer, config::VERSION, out.use_color)?;
    out.writeln("")?;

    let game_kind = resolve_game_kind(&args, input, out)?;
    let game_path = resolve_game_path(&args, game_kind, input, out)?;
    let selected_mods = resolve_setup_mods(&args, game_kind, input, out)?;
    let width = args.width.unwrap_or(DEFAULT_WIDTH);
    let height = args.height.unwrap_or(DEFAULT_HEIGHT);

    out.writeln(&format!(
        "Setting up {} at {}",
        game_kind.name(),
        out.path(&game_path.display().to_string())
    ))?;
    out.writeln("")?;

    Builder::new_current_thread()
        .build()?
        .block_on(common::install_runtimes(
            game_path.clone(),
            game_kind.app_id(),
        ))?;

    if game_kind == GameKind::SADX {
        sadx::convert_steam_to_2004(&game_path, None)?;
    }

    common::install_mod_manager(&game_path, game_kind, None)?;
    pipeline::install_selected_mods_and_generate_config(
        &game_path,
        game_kind,
        &selected_mods,
        width,
        height,
    )?;

    out.writeln("")?;
    out.success("Setup complete!")?;
    Ok(())
}

fn resolve_game_kind(
    args: &SetupArgs,
    input: &mut impl BufRead,
    out: &mut CliOutput,
) -> Result<GameKind> {
    if let Some(game) = &args.game {
        return parse_game_kind(game);
    }

    if args.game_path.is_some() {
        out.prompt("Select game [sadx/sa2]:")?;
        return parse_game_kind(&read_prompt(input)?);
    }

    let result = detect_games_strict(&args.detect)?;
    if result.games.len() == 1 {
        return Ok(result.games[0].kind);
    }

    if result.games.is_empty() {
        bail!("No supported games detected. Pass --game and --game-path.");
    }

    out.heading("Select installation:")?;
    for (index, game) in result.games.iter().enumerate() {
        out.writeln(&format!(
            "{}. {} ({})",
            index + 1,
            game.kind.name(),
            out.path(&game.path.display().to_string())
        ))?;
    }

    let selected = read_prompt(input)?;
    let index = selected
        .parse::<usize>()
        .context("Expected an installation number")?;
    let game = result
        .games
        .get(index.saturating_sub(1))
        .ok_or_else(|| anyhow!("Invalid installation number"))?;
    Ok(game.kind)
}

fn validate_game_path(game_kind: GameKind, path: &std::path::Path) -> Result<()> {
    anyhow::ensure!(
        path.is_dir(),
        "Game path does not exist or is not a directory: {}",
        path.display()
    );

    let has_game_marker = match game_kind {
        GameKind::SADX => {
            path.join("sonic.exe").is_file() || path.join("Sonic Adventure DX.exe").is_file()
        }
        GameKind::SA2 => path.join("sonic2app.exe").is_file(),
    };

    let dir_matches = path
        .file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n == game_kind.install_dir());

    if !has_game_marker && !dir_matches {
        bail!(
            "{} does not appear to be a {} installation (expected directory named '{}' or game executable)",
            path.display(),
            game_kind.name(),
            game_kind.install_dir(),
        );
    }

    Ok(())
}

fn resolve_game_path(
    args: &SetupArgs,
    game_kind: GameKind,
    input: &mut impl BufRead,
    out: &mut CliOutput,
) -> Result<PathBuf> {
    if let Some(path) = &args.game_path {
        validate_game_path(game_kind, path)?;
        return Ok(path.clone());
    }

    let mut games: Vec<Game> = detect_games_strict(&args.detect)?
        .games
        .into_iter()
        .filter(|game| game.kind == game_kind)
        .collect();

    match games.len() {
        0 => bail!("{} was not detected. Pass --game-path.", game_kind.name()),
        1 => Ok(games.remove(0).path),
        _ => {
            out.heading(&format!("Select {} installation:", game_kind.name()))?;
            for (index, game) in games.iter().enumerate() {
                out.writeln(&format!(
                    "{}. {}",
                    index + 1,
                    out.path(&game.path.display().to_string())
                ))?;
            }
            let selected = read_prompt(input)?;
            let index = selected
                .parse::<usize>()
                .context("Expected an installation number")?;
            let game = games
                .get(index.saturating_sub(1))
                .ok_or_else(|| anyhow!("Invalid installation number"))?;
            Ok(game.path.clone())
        }
    }
}

fn resolve_setup_mods(
    args: &SetupArgs,
    game_kind: GameKind,
    input: &mut impl BufRead,
    out: &mut CliOutput,
) -> Result<Vec<&'static common::ModEntry>> {
    let named_mods: Vec<&str> = args.mods.iter().map(String::as_str).collect();
    let selected = pipeline::resolve_selected_mods(game_kind, args.preset.as_deref(), &named_mods)?;
    if !selected.is_empty() {
        return Ok(selected);
    }

    let presets = common::presets_for_game(game_kind);
    if !presets.is_empty() {
        out.heading("Select preset:")?;
        for (index, preset) in presets.iter().enumerate() {
            out.writeln(&format!("{}. {}", index + 1, preset.name))?;
        }
        out.writeln(&format!("{}. Custom mod list", presets.len() + 1))?;
        let selected = read_prompt(input)?;
        let index = selected
            .parse::<usize>()
            .context("Expected a preset number")?;
        if (1..=presets.len()).contains(&index) {
            return pipeline::resolve_selected_mods(game_kind, Some(presets[index - 1].name), &[]);
        }
        if index != presets.len() + 1 {
            bail!("Invalid preset number");
        }
    }

    prompt_for_custom_mods(game_kind, input, out)
}

fn prompt_for_custom_mods(
    game_kind: GameKind,
    input: &mut impl BufRead,
    out: &mut CliOutput,
) -> Result<Vec<&'static common::ModEntry>> {
    let mods = common::recommended_mods_for_game(game_kind);
    out.heading("Select mods as comma-separated numbers:")?;
    for (index, mod_entry) in mods.iter().enumerate() {
        out.writeln(&format!("{}. {}", index + 1, mod_entry.name))?;
    }
    let selected = read_prompt(input)?;
    if selected.is_empty() {
        return Ok(mods.iter().collect());
    }

    selected
        .split(',')
        .map(|part| {
            let index = part
                .trim()
                .parse::<usize>()
                .context("Expected a mod number")?;
            mods.get(index.saturating_sub(1))
                .ok_or_else(|| anyhow!("Invalid mod number {}", index))
        })
        .collect()
}

fn detect_games_strict(args: &DetectArgs) -> Result<DetectionResult> {
    match &args.libraryfolders_vdf {
        Some(path) => {
            anyhow::ensure!(
                path.is_file(),
                "Library folders file not found: {}",
                path.display()
            );
            library::detect_games_from_vdf_strict(path, &args.steam_libraries)
        }
        None => Ok(library::detect_games_with_extra_libraries(
            &args.steam_libraries,
        )),
    }
}

fn parse_game_kind(raw: &str) -> Result<GameKind> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "sadx" => Ok(GameKind::SADX),
        "sa2" => Ok(GameKind::SA2),
        other => bail!("Unknown game '{}'. Use 'sadx' or 'sa2'.", other),
    }
}

fn read_prompt(input: &mut impl BufRead) -> Result<String> {
    let mut line = String::new();
    let bytes = input.read_line(&mut line)?;
    if bytes == 0 {
        bail!("Interactive input required but stdin is empty")
    }
    Ok(line.trim().to_string())
}

pub fn looks_like_cli(args: &[String]) -> bool {
    args.len() > 1
        && matches!(
            args[1].as_str(),
            "detect" | "list-mods" | "setup" | "--help" | "-h" | "--version" | "-V"
        )
}

pub fn run_from_args(args: Vec<String>) -> Result<bool> {
    let is_terminal = std::io::stdout().is_terminal();
    run_from_args_with_io(
        args,
        initialize_crypto_provider,
        &mut std::io::stdin(),
        &mut std::io::stdout(),
        is_terminal,
    )
}

pub fn run_from_args_with_io(
    args: Vec<String>,
    initialize_runtime: impl FnOnce() -> Result<()>,
    input: &mut impl Read,
    output: &mut impl Write,
    is_terminal: bool,
) -> Result<bool> {
    if !looks_like_cli(&args) {
        return Ok(false);
    }

    let cli = match Cli::try_parse_from(&args) {
        Ok(cli) => cli,
        Err(error) => match error.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                write!(output, "{error}")?;
                return Ok(true);
            }
            _ => return Err(anyhow!(error.to_string().trim_end().to_string())),
        },
    };

    let use_color = !cli.no_color && std::env::var("NO_COLOR").is_err() && is_terminal;

    initialize_runtime()?;

    run_with_io(cli, use_color, input, output)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use clap::Parser;

    use super::{resolve_setup_mods, run_from_args_with_io, Cli, CliOutput, Command, SetupArgs};
    use crate::steam::game::GameKind;

    #[test]
    fn parses_detect_command() {
        let cli = Cli::parse_from(["adventure-mods", "detect"]);
        assert!(matches!(cli.command, Some(Command::Detect(_))));
    }

    #[test]
    fn parses_list_mods_command() {
        let cli = Cli::parse_from(["adventure-mods", "list-mods", "--game", "sadx"]);

        match cli.command {
            Some(Command::ListMods { game }) => assert_eq!(game.as_str(), "sadx"),
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_setup_command_with_repeated_mods() {
        let cli = Cli::parse_from([
            "adventure-mods",
            "setup",
            "--game",
            "sa2",
            "--mod",
            "SA2 Render Fix",
            "--mod",
            "HD GUI: SA2 Edition",
            "--width",
            "1280",
            "--height",
            "720",
        ]);

        match cli.command {
            Some(Command::Setup(args)) => {
                assert_eq!(args.game.as_deref(), Some("sa2"));
                assert_eq!(args.mods, vec!["SA2 Render Fix", "HD GUI: SA2 Edition"]);
                assert_eq!(args.width, Some(1280));
                assert_eq!(args.height, Some(720));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_no_color_flag() {
        let cli = Cli::parse_from(["adventure-mods", "--no-color", "detect"]);
        assert!(cli.no_color);
    }

    #[test]
    fn run_from_args_initializes_runtime_for_cli_commands() {
        let mut initialized = false;
        let mut output = Vec::new();

        let handled = run_from_args_with_io(
            vec!["adventure-mods".to_string(), "detect".to_string()],
            || {
                initialized = true;
                Ok(())
            },
            &mut std::io::empty(),
            &mut output,
            false,
        )
        .unwrap();

        assert!(handled);
        assert!(initialized);
    }

    #[test]
    fn setup_rejects_zero_preset_selection() {
        let args = SetupArgs {
            game: Some("sadx".to_string()),
            mods: Vec::new(),
            preset: None,
            width: None,
            height: None,
            game_path: None,
            detect: Default::default(),
        };
        let mut input = std::io::Cursor::new(b"0\n");
        let mut output = Vec::new();
        let mut out = CliOutput::new(&mut output as &mut dyn Write, false);

        let error = match resolve_setup_mods(&args, GameKind::SADX, &mut input, &mut out) {
            Ok(_) => panic!("preset selection should reject 0"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("Invalid preset number"));
    }

    #[test]
    fn run_from_args_ignores_unknown_positional_args() {
        let mut initialized = false;
        let mut output = Vec::new();

        let handled = run_from_args_with_io(
            vec!["adventure-mods".to_string(), "detcet".to_string()],
            || {
                initialized = true;
                Ok(())
            },
            &mut std::io::empty(),
            &mut output,
            false,
        )
        .unwrap();

        assert!(!handled);
        assert!(!initialized);
    }

    #[test]
    fn looks_like_cli_matches_known_subcommands() {
        assert!(super::looks_like_cli(&[
            "adventure-mods".to_string(),
            "detect".to_string()
        ]));
        assert!(super::looks_like_cli(&[
            "adventure-mods".to_string(),
            "list-mods".to_string()
        ]));
        assert!(super::looks_like_cli(&[
            "adventure-mods".to_string(),
            "setup".to_string()
        ]));
        assert!(super::looks_like_cli(&[
            "adventure-mods".to_string(),
            "--help".to_string()
        ]));
        assert!(super::looks_like_cli(&[
            "adventure-mods".to_string(),
            "--version".to_string()
        ]));
    }

    #[test]
    fn looks_like_cli_ignores_unknown_args() {
        assert!(!super::looks_like_cli(&[
            "adventure-mods".to_string(),
            "/some/path".to_string()
        ]));
        assert!(!super::looks_like_cli(&[
            "adventure-mods".to_string(),
            "-g".to_string()
        ]));
        assert!(!super::looks_like_cli(&["adventure-mods".to_string()]));
    }

    #[test]
    fn validate_game_path_rejects_missing_directory() {
        let path = std::path::PathBuf::from("/nonexistent/path/Sonic Adventure DX");
        let result = super::validate_game_path(GameKind::SADX, &path);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("does not exist"));
    }

    #[test]
    fn validate_game_path_rejects_wrong_game_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let sa2_path = tmp.path().join("Sonic Adventure 2");
        std::fs::create_dir_all(&sa2_path).unwrap();

        let result = super::validate_game_path(GameKind::SADX, &sa2_path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("does not appear to be"));
    }

    #[test]
    fn validate_game_path_accepts_matching_directory_name() {
        let tmp = tempfile::tempdir().unwrap();
        let sadx_path = tmp.path().join("Sonic Adventure DX");
        std::fs::create_dir_all(&sadx_path).unwrap();

        let result = super::validate_game_path(GameKind::SADX, &sadx_path);
        assert!(result.is_ok());
    }

    #[test]
    fn cli_output_no_color_writes_plain() {
        let mut buf = Vec::new();
        let mut out = CliOutput::new(&mut buf as &mut dyn Write, false);

        out.heading("Test Heading").unwrap();
        out.success("Test Success").unwrap();
        out.bold_item("name", "desc").unwrap();

        let s = String::from_utf8(buf).unwrap();
        assert!(s.contains("Test Heading"));
        assert!(s.contains("Test Success"));
        assert!(s.contains("name: desc"));
        assert!(!s.contains("\x1b["));
    }
}
