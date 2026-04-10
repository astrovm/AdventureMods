use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
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

    fn writeln(&mut self, s: &str) -> std::io::Result<()> {
        writeln!(self.writer, "{s}")
    }
}

trait Prompt {
    fn select(&self, prompt: &str, items: &[String], default: usize) -> Result<usize>;
    fn multi_select(&self, prompt: &str, items: &[String], defaults: &[bool])
        -> Result<Vec<usize>>;
    fn confirm(&self, prompt: &str, default: bool) -> Result<bool>;
}

struct TerminalPrompt {
    use_color: bool,
}

impl TerminalPrompt {
    fn with_stderr_colors<T>(&self, action: impl FnOnce() -> T) -> T {
        let previous = console::colors_enabled_stderr();
        console::set_colors_enabled_stderr(self.use_color);
        let result = action();
        console::set_colors_enabled_stderr(previous);
        result
    }
}

impl Prompt for TerminalPrompt {
    fn select(&self, prompt: &str, items: &[String], default: usize) -> Result<usize> {
        self.with_stderr_colors(|| {
            Ok(Select::with_theme(&prompt_theme())
                .with_prompt(prompt)
                .items(items)
                .default(default)
                .interact()?)
        })
    }

    fn multi_select(
        &self,
        prompt: &str,
        items: &[String],
        defaults: &[bool],
    ) -> Result<Vec<usize>> {
        self.with_stderr_colors(|| {
            Ok(MultiSelect::with_theme(&prompt_theme())
                .with_prompt(prompt)
                .items(items)
                .defaults(defaults)
                .interact()?)
        })
    }

    fn confirm(&self, prompt: &str, default: bool) -> Result<bool> {
        self.with_stderr_colors(|| {
            Ok(Confirm::with_theme(&prompt_theme())
                .with_prompt(prompt)
                .default(default)
                .interact()?)
        })
    }
}

#[derive(Debug, Parser)]
#[command(name = "adventure-mods", version = config::VERSION)]
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
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    pub width: Option<u32>,
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    pub height: Option<u32>,
    #[arg(long)]
    pub game_path: Option<PathBuf>,
    #[command(flatten)]
    pub detect: DetectArgs,
}

pub fn run_with_io(cli: Cli, use_color: bool, output: &mut impl Write) -> Result<()> {
    let mut out = CliOutput::new(output, use_color);

    match cli.command {
        Some(Command::Detect(args)) => run_detect(args, &mut out),
        Some(Command::ListMods { game }) => run_list_mods(&game, &mut out),
        Some(Command::Setup(args)) => run_setup(args, &mut out),
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

fn run_setup(args: SetupArgs, out: &mut CliOutput) -> Result<()> {
    let term_width = console::Term::stdout().size().1 as usize;
    if term_width >= 30 {
        banner::print_banner(&mut out.writer, out.use_color)?;
    }
    banner::print_header(&mut out.writer, config::VERSION, out.use_color)?;
    out.writeln("")?;

    let rich_prompts = use_rich_prompts(&args);
    let prompt = TerminalPrompt {
        use_color: out.use_color,
    };

    // Detect once so both kind and path resolution share the same result.
    let detected = if rich_prompts && args.game.is_none() && args.game_path.is_none() {
        Some(detect_games_strict(&args.detect)?)
    } else {
        None
    };

    let game_kind = if rich_prompts {
        resolve_game_kind_rich(&args, detected.as_ref(), &prompt)?
    } else {
        resolve_game_kind(&args)?
    };
    let game_path = if rich_prompts {
        resolve_game_path_rich(&args, game_kind, detected.as_ref(), &prompt)?
    } else {
        resolve_game_path(&args, game_kind)?
    };
    let selected_mods = if rich_prompts {
        resolve_setup_mods_rich(&args, game_kind, &prompt)?
    } else {
        resolve_setup_mods(&args, game_kind)?
    };
    let width = args.width.unwrap_or(DEFAULT_WIDTH);
    let height = args.height.unwrap_or(DEFAULT_HEIGHT);

    if rich_prompts {
        confirm_setup_summary(
            out,
            game_kind,
            &game_path,
            &selected_mods,
            width,
            height,
            &prompt,
        )?;
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

fn resolve_game_kind_rich(
    args: &SetupArgs,
    detected: Option<&DetectionResult>,
    prompt: &dyn Prompt,
) -> Result<GameKind> {
    if let Some(game) = &args.game {
        return parse_game_kind(game);
    }

    if args.game_path.is_some() {
        let options = [GameKind::SADX, GameKind::SA2];
        let labels: Vec<String> = options.iter().map(|g| g.name().to_string()).collect();
        let selection = prompt.select("Select game", &labels, 0)?;
        return Ok(options[selection]);
    }

    let result = match detected {
        Some(r) => r,
        None => &detect_games_strict(&args.detect)?,
    };
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
    let selection = prompt.select("Select installation", &items, 0)?;
    Ok(result.games[selection].kind)
}

fn resolve_game_path_rich(
    args: &SetupArgs,
    game_kind: GameKind,
    detected: Option<&DetectionResult>,
    prompt: &dyn Prompt,
) -> Result<PathBuf> {
    if let Some(path) = &args.game_path {
        validate_game_path(game_kind, path)?;
        return Ok(path.clone());
    }

    let mut games: Vec<Game> = match detected {
        Some(r) => r
            .games
            .iter()
            .filter(|g| g.kind == game_kind)
            .cloned()
            .collect(),
        None => detect_games_strict(&args.detect)?
            .games
            .into_iter()
            .filter(|game| game.kind == game_kind)
            .collect(),
    };

    match games.len() {
        0 => bail!("{} was not detected. Pass --game-path.", game_kind.name()),
        1 => Ok(games.remove(0).path),
        _ => {
            let items: Vec<String> = games
                .iter()
                .map(|game| game.path.display().to_string())
                .collect();
            let selection = prompt.select(
                &format!("Select {} installation", game_kind.name()),
                &items,
                0,
            )?;
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

    if let Some(preset_name) = &args.preset {
        let selected = pipeline::resolve_selected_mods(game_kind, Some(preset_name.as_str()), &[])?;
        return Ok(Some(selected));
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
    prompt: &dyn Prompt,
) -> Result<Vec<&'static common::ModEntry>> {
    if let Some(selected) = resolve_setup_mods_from_flags(args, game_kind)? {
        return Ok(selected);
    }

    let presets = common::presets_for_game(game_kind);
    let mut options = Vec::new();
    for preset in presets {
        options.push(format!("{} - {}", preset.name, preset.description));
    }
    let has_all_recommended_option = presets.is_empty();
    if has_all_recommended_option {
        options.push("Install all recommended mods".to_string());
    }
    options.push("Choose mods manually".to_string());

    let selection = prompt.select("Choose setup mode", &options, 0)?;

    if selection < presets.len() {
        return pipeline::resolve_selected_mods(game_kind, Some(presets[selection].name), &[]);
    }
    if has_all_recommended_option && selection == presets.len() {
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
    let selections = prompt.multi_select(
        "Select mods (space to toggle, enter to confirm)",
        &items,
        &defaults,
    )?;

    if selections.is_empty() {
        bail!("Select at least one mod.");
    }

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
    prompt: &dyn Prompt,
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

    if !prompt.confirm("Proceed with setup?", true)? {
        bail!("Setup cancelled");
    }

    Ok(())
}

fn resolve_game_kind(args: &SetupArgs) -> Result<GameKind> {
    if let Some(game) = &args.game {
        return parse_game_kind(game);
    }

    if args.game_path.is_some() {
        bail!(
            "--game-path requires --game in non-interactive mode.\nPass --game sadx or --game sa2."
        );
    }

    let result = detect_games_strict(&args.detect)?;
    if result.games.is_empty() {
        bail!("No supported games detected. Pass --game and --game-path.");
    }
    if result.games.len() == 1 {
        return Ok(result.games[0].kind);
    }

    bail!(
        "Multiple games detected ({}). Pass --game to select one.",
        result
            .games
            .iter()
            .map(|g| g.kind.name())
            .collect::<Vec<_>>()
            .join(", ")
    );
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

fn resolve_game_path(args: &SetupArgs, game_kind: GameKind) -> Result<PathBuf> {
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
        _ => bail!(
            "Multiple {} installations detected. Pass --game-path to select one.",
            game_kind.name()
        ),
    }
}

fn resolve_setup_mods(
    args: &SetupArgs,
    game_kind: GameKind,
) -> Result<Vec<&'static common::ModEntry>> {
    if let Some(selected) = resolve_setup_mods_from_flags(args, game_kind)? {
        return Ok(selected);
    }

    bail!(
        "No mod selection specified. Pass --all-mods, --preset <name>, or --mods <id1,id2>.\n\
         Use 'list-mods --game {}' to see available mods and presets.",
        game_kind_arg(game_kind)
    )
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

fn gui_flag_name(arg: &str) -> &str {
    arg.split_once('=').map(|(flag, _)| flag).unwrap_or(arg)
}

fn is_known_gui_flag(arg: &str) -> bool {
    let flag = gui_flag_name(arg);
    matches!(
        flag,
        "--display" | "--class" | "--name" | "--sync" | "--g-fatal-warnings"
    ) || flag.starts_with("--gapplication-")
        || flag.starts_with("--gtk-")
        || flag.starts_with("--gdk-")
}

fn gui_flag_requires_value(arg: &str) -> bool {
    if arg.contains('=') {
        return false;
    }

    matches!(
        gui_flag_name(arg),
        "--display"
            | "--class"
            | "--name"
            | "--gapplication-app-id"
            | "--gtk-debug"
            | "--gdk-debug"
    )
}

fn has_only_gui_flags(args: &[String]) -> bool {
    let mut index = 1;

    while index < args.len() {
        let arg = &args[index];
        if !arg.starts_with('-') || !is_known_gui_flag(arg) {
            return false;
        }

        if gui_flag_requires_value(arg) {
            index += 1;
            if index >= args.len() {
                return false;
            }
        }

        index += 1;
    }

    true
}

pub fn looks_like_cli(args: &[String]) -> bool {
    args.len() > 1 && !has_only_gui_flags(args)
}

pub fn run_from_args(args: Vec<String>) -> Result<bool> {
    let is_terminal = std::io::stdout().is_terminal();
    run_from_args_with_io(
        args,
        initialize_crypto_provider,
        &mut std::io::stdout(),
        is_terminal,
    )
}

pub fn run_from_args_with_io(
    args: Vec<String>,
    initialize_runtime: impl FnOnce() -> Result<()>,
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

    run_with_io(cli, use_color, output)?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::{Mutex, OnceLock};

    use clap::Parser;

    use super::{
        resolve_game_kind_rich, resolve_setup_mods, resolve_setup_mods_rich, run_from_args_with_io,
        setup_is_fully_specified, Cli, CliOutput, Command, Prompt, SetupArgs, TerminalPrompt,
    };
    use crate::setup::common;
    use crate::steam::game::GameKind;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct MockPrompt {
        select_result: usize,
        multi_select_result: Vec<usize>,
        confirm_result: bool,
    }

    impl Prompt for MockPrompt {
        fn select(
            &self,
            _prompt: &str,
            _items: &[String],
            _default: usize,
        ) -> anyhow::Result<usize> {
            Ok(self.select_result)
        }

        fn multi_select(
            &self,
            _prompt: &str,
            _items: &[String],
            _defaults: &[bool],
        ) -> anyhow::Result<Vec<usize>> {
            Ok(self.multi_select_result.clone())
        }

        fn confirm(&self, _prompt: &str, _default: bool) -> anyhow::Result<bool> {
            Ok(self.confirm_result)
        }
    }

    #[test]
    fn resolve_game_kind_rich_uses_game_flag() {
        let args = SetupArgs {
            game: Some("sa2".to_string()),
            mods: None,
            preset: None,
            all_mods: false,
            width: None,
            height: None,
            game_path: None,
            detect: Default::default(),
        };
        let prompt = MockPrompt {
            select_result: 0,
            multi_select_result: vec![],
            confirm_result: true,
        };

        let kind = resolve_game_kind_rich(&args, None, &prompt).unwrap();
        assert_eq!(kind, GameKind::SA2);
    }

    #[test]
    fn resolve_setup_mods_rich_with_all_mods_flag() {
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
        let prompt = MockPrompt {
            select_result: 0,
            multi_select_result: vec![],
            confirm_result: true,
        };

        let selected = resolve_setup_mods_rich(&args, GameKind::SA2, &prompt).unwrap();
        assert_eq!(
            selected.len(),
            common::recommended_mods_for_game(GameKind::SA2).len()
        );
    }

    #[test]
    fn resolve_setup_mods_rich_hides_all_recommended_when_presets_exist() {
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
        let prompt = MockPrompt {
            select_result: 2,
            multi_select_result: vec![0],
            confirm_result: true,
        };

        let selected = resolve_setup_mods_rich(&args, GameKind::SADX, &prompt).unwrap();

        assert_eq!(selected.len(), 1);
        assert_eq!(
            selected[0].name,
            common::recommended_mods_for_game(GameKind::SADX)[0].name
        );
    }

    #[test]
    fn resolve_setup_mods_rich_rejects_empty_manual_selection() {
        let args = SetupArgs {
            game: Some("sa2".to_string()),
            mods: None,
            preset: None,
            all_mods: false,
            width: None,
            height: None,
            game_path: None,
            detect: Default::default(),
        };
        let prompt = MockPrompt {
            select_result: 1,
            multi_select_result: vec![],
            confirm_result: true,
        };

        let Err(error) = resolve_setup_mods_rich(&args, GameKind::SA2, &prompt) else {
            panic!("expected empty manual selection to fail");
        };

        assert!(error.to_string().contains("at least one mod"));
    }

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
    fn rejects_zero_width_flag() {
        let error = Cli::try_parse_from([
            "adventure-mods",
            "setup",
            "--game",
            "sa2",
            "--mods",
            "sa2-render-fix",
            "--width",
            "0",
        ])
        .unwrap_err();

        assert!(error.to_string().contains("--width"));
    }

    #[test]
    fn rejects_zero_height_flag() {
        let error = Cli::try_parse_from([
            "adventure-mods",
            "setup",
            "--game",
            "sa2",
            "--mods",
            "sa2-render-fix",
            "--height",
            "0",
        ])
        .unwrap_err();

        assert!(error.to_string().contains("--height"));
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
            &mut output,
            false,
        )
        .unwrap();

        assert!(handled);
        assert!(initialized);
    }

    #[test]
    fn resolve_setup_mods_bails_without_mod_selection() {
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

        let Err(error) = resolve_setup_mods(&args, GameKind::SADX) else {
            panic!("expected error when no mod selection specified");
        };

        assert!(error.to_string().contains("--all-mods"));
        assert!(error.to_string().contains("list-mods"));
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

        let selected = resolve_setup_mods(&args, GameKind::SA2).unwrap();

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

        let Err(error) = resolve_setup_mods(&args, GameKind::SADX) else {
            panic!("expected incompatible all-mods selection to fail");
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

        let selected = resolve_setup_mods(&args, GameKind::SA2).unwrap();
        let names: Vec<&str> = selected.iter().map(|entry| entry.name).collect();

        assert_eq!(names, vec!["SA2 Render Fix", "HD GUI: SA2 Edition"]);
    }

    #[test]
    fn resolve_setup_mods_accepts_preset_flag() {
        let args = SetupArgs {
            game: Some("sadx".to_string()),
            mods: None,
            preset: Some("DX Enhanced".to_string()),
            all_mods: false,
            width: None,
            height: None,
            game_path: None,
            detect: Default::default(),
        };

        let selected = resolve_setup_mods(&args, GameKind::SADX).unwrap();

        assert!(!selected.is_empty());
        assert!(selected.iter().any(|m| m.name == "Dreamcast Conversion"));
    }

    #[test]
    fn resolve_game_kind_requires_game_flag_when_game_path_given() {
        let args = SetupArgs {
            game: None,
            mods: Some("sa2-render-fix".to_string()),
            preset: None,
            all_mods: false,
            width: None,
            height: None,
            game_path: Some(PathBuf::from("/tmp/sa2")),
            detect: Default::default(),
        };

        let error = super::resolve_game_kind(&args).unwrap_err();
        assert!(error.to_string().contains("--game"));
    }

    #[test]
    fn run_from_args_surfaces_unknown_subcommand_as_error() {
        let mut output = Vec::new();

        let result = run_from_args_with_io(
            vec!["adventure-mods".to_string(), "detcet".to_string()],
            || Ok(()),
            &mut output,
            false,
        );

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("detcet"));
    }

    #[test]
    fn run_from_args_surfaces_unknown_top_level_flag_as_error() {
        let mut output = Vec::new();

        let result = run_from_args_with_io(
            vec!["adventure-mods".to_string(), "--bogus".to_string()],
            || Ok(()),
            &mut output,
            false,
        );

        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.to_string().contains("--bogus"));
    }

    #[test]
    fn run_from_args_ignores_gui_flags_that_take_separate_values() {
        let mut output = Vec::new();
        let mut initialized = false;

        let handled = run_from_args_with_io(
            vec![
                "adventure-mods".to_string(),
                "--gtk-debug".to_string(),
                "interactive".to_string(),
            ],
            || {
                initialized = true;
                Ok(())
            },
            &mut output,
            false,
        )
        .unwrap();

        assert!(!handled);
        assert!(!initialized);
    }

    #[test]
    fn run_from_args_help_does_not_initialize_runtime() {
        let mut output = Vec::new();
        let mut initialized = false;

        let handled = run_from_args_with_io(
            vec!["adventure-mods".to_string(), "--help".to_string()],
            || {
                initialized = true;
                Ok(())
            },
            &mut output,
            false,
        )
        .unwrap();

        assert!(handled);
        assert!(!initialized);
        assert!(String::from_utf8(output).unwrap().contains("Usage:"));
    }

    #[test]
    fn run_from_args_version_does_not_initialize_runtime() {
        let mut output = Vec::new();
        let mut initialized = false;

        let handled = run_from_args_with_io(
            vec!["adventure-mods".to_string(), "--version".to_string()],
            || {
                initialized = true;
                Ok(())
            },
            &mut output,
            false,
        )
        .unwrap();

        assert!(handled);
        assert!(!initialized);
        assert!(String::from_utf8(output)
            .unwrap()
            .contains(env!("CARGO_PKG_VERSION")));
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
    fn looks_like_cli_ignores_gui_flags() {
        assert!(!super::looks_like_cli(&[
            "adventure-mods".to_string(),
            "--gapplication-service".to_string()
        ]));
        assert!(!super::looks_like_cli(&[
            "adventure-mods".to_string(),
            "--display".to_string(),
            ":1".to_string()
        ]));
        assert!(!super::looks_like_cli(&[
            "adventure-mods".to_string(),
            "--gtk-debug".to_string(),
            "interactive".to_string()
        ]));
        assert!(!super::looks_like_cli(&[
            "adventure-mods".to_string(),
            "--gdk-debug=events".to_string()
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
    fn looks_like_cli_accepts_global_flags_before_subcommand() {
        assert!(super::looks_like_cli(&[
            "adventure-mods".to_string(),
            "--no-color".to_string(),
            "detect".to_string(),
        ]));
    }

    #[test]
    fn terminal_prompt_respects_no_color_setting() {
        console::set_colors_enabled_stderr(true);

        let prompt = TerminalPrompt { use_color: false };
        let during = prompt.with_stderr_colors(|| console::colors_enabled_stderr());

        assert!(!during);
        assert!(console::colors_enabled_stderr());
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

    #[test]
    fn cli_output_with_color_emits_ansi_sequences() {
        let previous = console::colors_enabled();
        console::set_colors_enabled(true);

        let mut buf = Vec::new();
        let mut out = CliOutput::new(&mut buf as &mut dyn Write, true);

        out.heading("Test Heading").unwrap();
        out.success("Test Success").unwrap();
        out.bold_item("name", "desc").unwrap();

        let s = String::from_utf8(buf).unwrap();
        console::set_colors_enabled(previous);
        assert!(s.contains("\x1b["));
    }

    #[test]
    fn run_from_args_uses_color_for_terminal_output_by_default() {
        let _guard = env_lock().lock().unwrap_or_else(|e| e.into_inner());
        let had_no_color = std::env::var("NO_COLOR").ok();
        unsafe {
            std::env::remove_var("NO_COLOR");
        }

        let previous = console::colors_enabled();
        console::set_colors_enabled(true);

        let mut output = Vec::new();

        let result = run_from_args_with_io(
            vec![
                "adventure-mods".to_string(),
                "list-mods".to_string(),
                "--game".to_string(),
                "sa2".to_string(),
            ],
            || Ok(()),
            &mut output,
            true,
        );

        console::set_colors_enabled(previous);
        unsafe {
            if let Some(val) = had_no_color {
                std::env::set_var("NO_COLOR", val);
            }
        }

        let handled = result.unwrap();
        assert!(handled);
        assert!(String::from_utf8(output).unwrap().contains("\x1b["));
    }

    #[test]
    fn run_from_args_disables_color_with_no_color_env() {
        let _guard = env_lock().lock().unwrap();
        unsafe {
            std::env::set_var("NO_COLOR", "1");
        }

        let mut output = Vec::new();
        let result = run_from_args_with_io(
            vec![
                "adventure-mods".to_string(),
                "list-mods".to_string(),
                "--game".to_string(),
                "sa2".to_string(),
            ],
            || Ok(()),
            &mut output,
            true,
        );

        unsafe {
            std::env::remove_var("NO_COLOR");
        }

        let handled = result.unwrap();
        assert!(handled);
        assert!(!String::from_utf8(output).unwrap().contains("\x1b["));
    }
}
