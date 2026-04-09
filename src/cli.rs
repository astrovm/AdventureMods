use std::io::{BufRead, BufReader, IsTerminal, Read, Write};
use std::path::PathBuf;

use anyhow::{anyhow, bail, Context, Result};
use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand};
use console::Style;
use dialoguer::{theme::ColorfulTheme, Confirm, MultiSelect, Select};
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
    #[arg(long)]
    pub mods: Option<String>,
    #[arg(long)]
    pub preset: Option<String>,
    #[arg(long)]
    pub all_mods: bool,
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
        out.bold_item(
            &format!("{} [{}]", mod_entry.name, mod_cli_id(mod_entry.name)),
            mod_entry.description,
        )?;
    }

    Ok(())
}

fn run_setup(args: SetupArgs, input: &mut impl BufRead, out: &mut CliOutput) -> Result<()> {
    let term_width = console::Term::stdout().size().1 as usize;
    if term_width >= 30 {
        banner::print_banner(&mut out.writer, out.use_color)?;
    }
    banner::print_header(&mut out.writer, config::VERSION, out.use_color)?;
    out.writeln("")?;

    let rich_prompts = use_rich_prompts(&args);
    let game_kind = if rich_prompts {
        resolve_game_kind_rich(&args)?
    } else {
        resolve_game_kind(&args, input, out)?
    };
    let game_path = if rich_prompts {
        resolve_game_path_rich(&args, game_kind)?
    } else {
        resolve_game_path(&args, game_kind, input, out)?
    };
    let selected_mods = if rich_prompts {
        resolve_setup_mods_rich(&args, game_kind)?
    } else {
        resolve_setup_mods(&args, game_kind, input, out)?
    };
    let width = args.width.unwrap_or(DEFAULT_WIDTH);
    let height = args.height.unwrap_or(DEFAULT_HEIGHT);

    if rich_prompts {
        confirm_setup_summary(out, game_kind, &game_path, &selected_mods, width, height)?;
    }

    out.writeln(&format!(
        "Setting up {} at {}",
        game_kind.name(),
        out.path(&game_path.display().to_string())
    ))?;
    out.writeln("")?;

    let total_steps = total_setup_steps(game_kind);
    let mut step_index = 1;

    run_setup_step(out, step_index, total_steps, "Install .NET Runtime", || {
        Builder::new_current_thread()
            .build()?
            .block_on(common::install_runtimes(
                game_path.clone(),
                game_kind.app_id(),
            ))
    })?;
    step_index += 1;

    if game_kind == GameKind::SADX {
        run_setup_step(
            out,
            step_index,
            total_steps,
            "Convert Steam to 2004",
            || sadx::convert_steam_to_2004(&game_path, None),
        )?;
        step_index += 1;
    }

    run_setup_step(
        out,
        step_index,
        total_steps,
        "Install Mod Manager & Loader",
        || common::install_mod_manager(&game_path, game_kind, None),
    )?;
    step_index += 1;

    out.heading(&step_heading(
        step_index,
        total_steps,
        "Install Mods & Generate Config",
    ))?;
    pipeline::install_selected_mods_and_generate_config_with_progress(
        &game_path,
        game_kind,
        &selected_mods,
        width,
        height,
        |progress| match progress {
            pipeline::InstallProgress::InstallingMod {
                index,
                total,
                mod_name,
            } => {
                let _ = out.writeln(&format!("  - Installing mod {index}/{total}: {mod_name}"));
            }
            pipeline::InstallProgress::GeneratingConfig => {
                let _ = out.writeln("  - Generating mod config");
            }
        },
    )?;
    out.writeln("Done")?;

    out.writeln("")?;
    out.success("Setup complete!")?;
    Ok(())
}

fn use_rich_prompts(args: &SetupArgs) -> bool {
    std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal()
        && !setup_is_fully_specified(args)
}

fn setup_is_fully_specified(args: &SetupArgs) -> bool {
    args.game.is_some()
        && args.game_path.is_some()
        && (args.preset.is_some() || args.all_mods || args.mods.is_some())
}

fn total_setup_steps(game_kind: GameKind) -> usize {
    if game_kind == GameKind::SADX {
        4
    } else {
        3
    }
}

fn step_heading(index: usize, total: usize, label: &str) -> String {
    format!("Step {index}/{total}: {label}")
}

fn run_setup_step<T>(
    out: &mut CliOutput,
    index: usize,
    total: usize,
    label: &str,
    action: impl FnOnce() -> Result<T>,
) -> Result<T> {
    let heading = step_heading(index, total, label);
    out.heading(&heading)?;
    let value = action()?;
    out.writeln("Done")?;
    out.writeln("")?;
    Ok(value)
}

fn prompt_theme() -> ColorfulTheme {
    ColorfulTheme::default()
}

fn resolve_game_kind_rich(args: &SetupArgs) -> Result<GameKind> {
    if let Some(game) = &args.game {
        return parse_game_kind(game);
    }

    if args.game_path.is_some() {
        let options = [GameKind::SADX, GameKind::SA2];
        let labels = [GameKind::SADX.name(), GameKind::SA2.name()];
        let selection = Select::with_theme(&prompt_theme())
            .with_prompt("Select game")
            .items(&labels)
            .default(0)
            .interact()?;
        return Ok(options[selection]);
    }

    let result = detect_games_strict(&args.detect)?;
    if result.games.len() == 1 {
        return Ok(result.games[0].kind);
    }
    if result.games.is_empty() {
        bail!("No supported games detected. Pass --game and --game-path.");
    }

    let items: Vec<String> = result
        .games
        .iter()
        .map(|game| format!("{} ({})", game.kind.name(), game.path.display()))
        .collect();
    let selection = Select::with_theme(&prompt_theme())
        .with_prompt("Select installation")
        .items(&items)
        .default(0)
        .interact()?;
    Ok(result.games[selection].kind)
}

fn resolve_game_path_rich(args: &SetupArgs, game_kind: GameKind) -> Result<PathBuf> {
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
            let items: Vec<String> = games
                .iter()
                .map(|game| game.path.display().to_string())
                .collect();
            let selection = Select::with_theme(&prompt_theme())
                .with_prompt(format!("Select {} installation", game_kind.name()))
                .items(&items)
                .default(0)
                .interact()?;
            Ok(games[selection].path.clone())
        }
    }
}

fn resolve_setup_mods_from_flags(
    args: &SetupArgs,
    game_kind: GameKind,
) -> Result<Option<Vec<&'static common::ModEntry>>> {
    if args.mods.is_some() && args.preset.is_some() {
        bail!("Cannot use both --preset and --mods at the same time");
    }
    if args.all_mods && args.preset.is_some() {
        bail!("Cannot use --all-mods with --preset");
    }
    if args.all_mods && args.mods.is_some() {
        bail!("Cannot use --all-mods with --mods");
    }
    if args.all_mods {
        return Ok(Some(
            common::recommended_mods_for_game(game_kind)
                .iter()
                .collect(),
        ));
    }

    if let Some(mods) = &args.mods {
        let selected = resolve_mods_flag(game_kind, mods)?;
        return Ok(Some(selected));
    }

    Ok(None)
}

fn resolve_mods_flag(game_kind: GameKind, mods: &str) -> Result<Vec<&'static common::ModEntry>> {
    parse_mods_flag(mods)?
        .into_iter()
        .map(|identifier| resolve_mod_identifier(game_kind, identifier))
        .collect()
}

fn parse_mods_flag(mods: &str) -> Result<Vec<&str>> {
    let parsed: Vec<&str> = mods
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();

    if parsed.is_empty() {
        bail!("--mods requires at least one mod id");
    }

    Ok(parsed)
}

fn resolve_mod_identifier(
    game_kind: GameKind,
    identifier: &str,
) -> Result<&'static common::ModEntry> {
    common::recommended_mods_for_game(game_kind)
        .iter()
        .find(|mod_entry| {
            mod_entry.name.eq_ignore_ascii_case(identifier)
                || mod_cli_id(mod_entry.name).eq_ignore_ascii_case(identifier)
        })
        .ok_or_else(|| {
            anyhow!(
                "Unknown mod id '{}'. Use 'list-mods --game {}' to see valid ids.",
                identifier,
                game_kind_arg(game_kind)
            )
        })
}

fn mod_cli_id(name: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_dash = false;

    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            previous_was_dash = false;
        } else if !previous_was_dash {
            slug.push('-');
            previous_was_dash = true;
        }
    }

    slug.trim_matches('-').to_string()
}

fn game_kind_arg(game_kind: GameKind) -> &'static str {
    match game_kind {
        GameKind::SADX => "sadx",
        GameKind::SA2 => "sa2",
    }
}

fn resolve_setup_mods_rich(
    args: &SetupArgs,
    game_kind: GameKind,
) -> Result<Vec<&'static common::ModEntry>> {
    if let Some(selected) = resolve_setup_mods_from_flags(args, game_kind)? {
        return Ok(selected);
    }

    let presets = common::presets_for_game(game_kind);
    let mut options = Vec::new();
    for preset in presets {
        options.push(format!("{} - {}", preset.name, preset.description));
    }
    options.push("Install all recommended mods".to_string());
    options.push("Choose mods manually".to_string());

    let selection = Select::with_theme(&prompt_theme())
        .with_prompt("Choose setup mode")
        .items(&options)
        .default(0)
        .interact()?;

    if selection < presets.len() {
        return pipeline::resolve_selected_mods(game_kind, Some(presets[selection].name), &[]);
    }
    if selection == presets.len() {
        return Ok(common::recommended_mods_for_game(game_kind)
            .iter()
            .collect());
    }

    let mods = common::recommended_mods_for_game(game_kind);
    let items: Vec<String> = mods
        .iter()
        .map(|mod_entry| format!("{} - {}", mod_entry.name, mod_entry.description))
        .collect();
    let defaults = vec![true; items.len()];
    let selections = MultiSelect::with_theme(&prompt_theme())
        .with_prompt("Select mods (space to toggle, enter to confirm)")
        .items(&items)
        .defaults(&defaults)
        .interact()?;

    selections
        .into_iter()
        .map(|index| {
            mods.get(index)
                .ok_or_else(|| anyhow!("Invalid mod selection {}", index + 1))
        })
        .collect()
}

fn confirm_setup_summary(
    out: &mut CliOutput,
    game_kind: GameKind,
    game_path: &std::path::Path,
    selected_mods: &[&common::ModEntry],
    width: u32,
    height: u32,
) -> Result<()> {
    out.heading("Summary")?;
    out.writeln(&format!("Game: {}", game_kind.name()))?;
    out.writeln(&format!(
        "Path: {}",
        out.path(&game_path.display().to_string())
    ))?;
    out.writeln(&format!("Resolution: {}x{}", width, height))?;
    out.writeln(&format!("Mods selected: {}", selected_mods.len()))?;
    for mod_entry in selected_mods {
        out.writeln(&format!("- {}", mod_entry.name))?;
    }
    out.writeln("")?;

    if !Confirm::with_theme(&prompt_theme())
        .with_prompt("Proceed with setup?")
        .default(true)
        .interact()?
    {
        bail!("Setup cancelled");
    }

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

    anyhow::ensure!(
        has_game_marker,
        "{} does not appear to be a {} installation (expected game executable)",
        path.display(),
        game_kind.name(),
    );

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
    if let Some(selected) = resolve_setup_mods_from_flags(args, game_kind)? {
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
    if args.len() <= 1 {
        return false;
    }
    let second = &args[1];
    second == "--help"
        || second == "-h"
        || second == "--version"
        || second == "-V"
        || !second.starts_with('-')
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
    use std::path::PathBuf;

    use clap::Parser;

    use super::{
        resolve_setup_mods, run_from_args_with_io, setup_is_fully_specified, Cli, CliOutput,
        Command, SetupArgs,
    };
    use crate::setup::common;
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
    fn parses_setup_command_with_mods_flag() {
        let cli = Cli::parse_from([
            "adventure-mods",
            "setup",
            "--game",
            "sa2",
            "--mods",
            "sa2-render-fix,hd-gui-sa2-edition",
            "--width",
            "1280",
            "--height",
            "720",
        ]);

        match cli.command {
            Some(Command::Setup(args)) => {
                assert_eq!(args.game.as_deref(), Some("sa2"));
                assert_eq!(
                    args.mods.as_deref(),
                    Some("sa2-render-fix,hd-gui-sa2-edition")
                );
                assert!(!args.all_mods);
                assert_eq!(args.width, Some(1280));
                assert_eq!(args.height, Some(720));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn rejects_removed_non_interactive_flag() {
        let error =
            Cli::try_parse_from(["adventure-mods", "setup", "--non-interactive"]).unwrap_err();

        assert!(error.to_string().contains("--non-interactive"));
    }

    #[test]
    fn setup_is_fully_specified_with_explicit_flags() {
        let args = SetupArgs {
            game: Some("sa2".to_string()),
            mods: Some("sa2-render-fix".to_string()),
            preset: None,
            all_mods: false,
            width: None,
            height: None,
            game_path: Some(PathBuf::from("/tmp/sa2")),
            detect: Default::default(),
        };

        assert!(setup_is_fully_specified(&args));
    }

    #[test]
    fn setup_is_not_fully_specified_without_mod_choice() {
        let args = SetupArgs {
            game: Some("sa2".to_string()),
            mods: None,
            preset: None,
            all_mods: false,
            width: None,
            height: None,
            game_path: Some(PathBuf::from("/tmp/sa2")),
            detect: Default::default(),
        };

        assert!(!setup_is_fully_specified(&args));
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
            mods: None,
            preset: None,
            all_mods: false,
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
    fn resolve_setup_mods_returns_all_recommended_mods() {
        let args = SetupArgs {
            game: Some("sa2".to_string()),
            mods: None,
            preset: None,
            all_mods: true,
            width: None,
            height: None,
            game_path: None,
            detect: Default::default(),
        };
        let mut input = std::io::Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();
        let mut out = CliOutput::new(&mut output as &mut dyn Write, false);

        let selected = resolve_setup_mods(&args, GameKind::SA2, &mut input, &mut out).unwrap();

        assert_eq!(
            selected.len(),
            common::recommended_mods_for_game(GameKind::SA2).len()
        );
    }

    #[test]
    fn resolve_setup_mods_rejects_all_mods_with_preset() {
        let args = SetupArgs {
            game: Some("sadx".to_string()),
            mods: None,
            preset: Some("DX Enhanced".to_string()),
            all_mods: true,
            width: None,
            height: None,
            game_path: None,
            detect: Default::default(),
        };
        let mut input = std::io::Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();
        let mut out = CliOutput::new(&mut output as &mut dyn Write, false);

        let error = match resolve_setup_mods(&args, GameKind::SADX, &mut input, &mut out) {
            Ok(_) => panic!("expected incompatible all-mods selection to fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("Cannot use --all-mods"));
    }

    #[test]
    fn resolve_setup_mods_accepts_single_mods_flag() {
        let args = SetupArgs {
            game: Some("sa2".to_string()),
            mods: Some("sa2-render-fix,hd-gui-sa2-edition".to_string()),
            preset: None,
            all_mods: false,
            width: None,
            height: None,
            game_path: None,
            detect: Default::default(),
        };
        let mut input = std::io::Cursor::new(Vec::<u8>::new());
        let mut output = Vec::new();
        let mut out = CliOutput::new(&mut output as &mut dyn Write, false);

        let selected = resolve_setup_mods(&args, GameKind::SA2, &mut input, &mut out).unwrap();
        let names: Vec<&str> = selected.iter().map(|entry| entry.name).collect();

        assert_eq!(names, vec!["SA2 Render Fix", "HD GUI: SA2 Edition"]);
    }

    #[test]
    fn run_from_args_surfaces_unknown_subcommand_as_error() {
        let mut output = Vec::new();

        let result = run_from_args_with_io(
            vec!["adventure-mods".to_string(), "detcet".to_string()],
            || Ok(()),
            &mut std::io::empty(),
            &mut output,
            false,
        );

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("detcet"));
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
    fn looks_like_cli_ignores_flags() {
        assert!(!super::looks_like_cli(&[
            "adventure-mods".to_string(),
            "-g".to_string()
        ]));
        assert!(!super::looks_like_cli(&["adventure-mods".to_string()]));
    }

    #[test]
    fn looks_like_cli_detects_any_positional_arg() {
        assert!(super::looks_like_cli(&[
            "adventure-mods".to_string(),
            "typo".to_string()
        ]));
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
    fn validate_game_path_accepts_directory_with_game_executable() {
        let tmp = tempfile::tempdir().unwrap();
        let sadx_path = tmp.path().join("Sonic Adventure DX");
        std::fs::create_dir_all(&sadx_path).unwrap();
        std::fs::File::create(sadx_path.join("sonic.exe")).unwrap();

        let result = super::validate_game_path(GameKind::SADX, &sadx_path);
        assert!(result.is_ok());
    }

    #[test]
    fn validate_game_path_rejects_directory_without_executable() {
        let tmp = tempfile::tempdir().unwrap();
        let sadx_path = tmp.path().join("Sonic Adventure DX");
        std::fs::create_dir_all(&sadx_path).unwrap();

        let result = super::validate_game_path(GameKind::SADX, &sadx_path);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("does not appear to be"));
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
