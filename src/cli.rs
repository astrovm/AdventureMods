use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand};
use tokio::runtime::Builder;

use crate::setup::{common, pipeline, sadx};
use crate::steam::game::{Game, GameKind};
use crate::steam::library::{self, DetectionResult};

const DEFAULT_WIDTH: u32 = 1920;
const DEFAULT_HEIGHT: u32 = 1080;

#[derive(Debug, Parser)]
#[command(name = "adventure-mods")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
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

pub fn run_with_io(cli: Cli, input: &mut impl Read, output: &mut impl Write) -> Result<()> {
    let mut input = BufReader::new(input);

    match cli.command {
        Some(Command::Detect(args)) => run_detect(args, output),
        Some(Command::ListMods { game }) => run_list_mods(&game, output),
        Some(Command::Setup(args)) => run_setup(args, &mut input, output),
        None => Ok(()),
    }
}

pub fn initialize_crypto_provider() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .map_err(|_| anyhow!("failed to install TLS crypto provider: already installed"))?;
    Ok(())
}

fn run_detect(args: DetectArgs, output: &mut impl Write) -> Result<()> {
    let result = detect_games_strict(&args)?;

    if result.games.is_empty() && result.inaccessible.is_empty() {
        writeln!(output, "No supported games detected.")?;
        return Ok(());
    }

    if !result.games.is_empty() {
        writeln!(output, "Detected games:")?;
        for game in &result.games {
            writeln!(output, "- {}: {}", game.kind.name(), game.path.display())?;
        }
    }

    if !result.inaccessible.is_empty() {
        writeln!(output, "Inaccessible Steam libraries:")?;
        for game in &result.inaccessible {
            writeln!(
                output,
                "- {}: {}",
                game.kind.name(),
                game.library_path.display()
            )?;
        }
    }

    Ok(())
}

fn run_list_mods(game: &str, output: &mut impl Write) -> Result<()> {
    let game_kind = parse_game_kind(game)?;
    writeln!(output, "Game: {}", game_kind.name())?;

    let presets = common::presets_for_game(game_kind);
    if !presets.is_empty() {
        writeln!(output, "Presets:")?;
        for preset in presets {
            writeln!(output, "- {}: {}", preset.name, preset.description)?;
        }
    }

    writeln!(output, "Mods:")?;
    for mod_entry in common::recommended_mods_for_game(game_kind) {
        writeln!(output, "- {}: {}", mod_entry.name, mod_entry.description)?;
    }

    Ok(())
}

fn run_setup(args: SetupArgs, input: &mut impl BufRead, output: &mut impl Write) -> Result<()> {
    let game_kind = resolve_game_kind(&args, input, output)?;
    let game_path = resolve_game_path(&args, game_kind, input, output)?;
    let selected_mods = resolve_setup_mods(&args, game_kind, input, output)?;
    let width = args.width.unwrap_or(DEFAULT_WIDTH);
    let height = args.height.unwrap_or(DEFAULT_HEIGHT);

    writeln!(
        output,
        "Setting up {} at {}",
        game_kind.name(),
        game_path.display()
    )?;

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

    writeln!(output, "Setup complete.")?;
    Ok(())
}

fn resolve_game_kind(
    args: &SetupArgs,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<GameKind> {
    if let Some(game) = &args.game {
        return parse_game_kind(game);
    }

    if args.game_path.is_some() {
        writeln!(output, "Select game: [sadx/sa2]")?;
        return parse_game_kind(&read_prompt(input)?);
    }

    let result = detect_games_strict(&args.detect)?;
    if result.games.len() == 1 {
        return Ok(result.games[0].kind);
    }

    if result.games.is_empty() {
        bail!("No supported games detected. Pass --game and --game-path.");
    }

    writeln!(output, "Select installation:")?;
    for (index, game) in result.games.iter().enumerate() {
        writeln!(
            output,
            "{}. {} ({})",
            index + 1,
            game.kind.name(),
            game.path.display()
        )?;
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
    output: &mut impl Write,
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
            writeln!(output, "Select {} installation:", game_kind.name())?;
            for (index, game) in games.iter().enumerate() {
                writeln!(output, "{}. {}", index + 1, game.path.display())?;
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
    output: &mut impl Write,
) -> Result<Vec<&'static common::ModEntry>> {
    let named_mods: Vec<&str> = args.mods.iter().map(String::as_str).collect();
    let selected = pipeline::resolve_selected_mods(game_kind, args.preset.as_deref(), &named_mods)?;
    if !selected.is_empty() {
        return Ok(selected);
    }

    let presets = common::presets_for_game(game_kind);
    if !presets.is_empty() {
        writeln!(output, "Select preset:")?;
        for (index, preset) in presets.iter().enumerate() {
            writeln!(output, "{}. {}", index + 1, preset.name)?;
        }
        writeln!(output, "{}. Custom mod list", presets.len() + 1)?;
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

    prompt_for_custom_mods(game_kind, input, output)
}

fn prompt_for_custom_mods(
    game_kind: GameKind,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<Vec<&'static common::ModEntry>> {
    let mods = common::recommended_mods_for_game(game_kind);
    writeln!(output, "Select mods as comma-separated numbers:")?;
    for (index, mod_entry) in mods.iter().enumerate() {
        writeln!(output, "{}. {}", index + 1, mod_entry.name)?;
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
    args.len() > 1 && matches!(args[1].as_str(), "detect" | "list-mods" | "setup")
}

pub fn run_from_args(args: Vec<String>) -> Result<bool> {
    run_from_args_with_io(
        args,
        initialize_crypto_provider,
        &mut std::io::stdin(),
        &mut std::io::stdout(),
    )
}

pub fn run_from_args_with_io(
    args: Vec<String>,
    initialize_runtime: impl FnOnce() -> Result<()>,
    input: &mut impl Read,
    output: &mut impl Write,
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

    initialize_runtime()?;

    run_with_io(cli, input, output)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{resolve_setup_mods, run_from_args_with_io, Cli, Command, SetupArgs};
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

        let error = match resolve_setup_mods(&args, GameKind::SADX, &mut input, &mut output) {
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
}
