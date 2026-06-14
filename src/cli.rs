use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use anyhow::{Result, anyhow, bail};
use clap::error::ErrorKind;
use clap::{Args, Parser, Subcommand};
use console::Style;
use dialoguer::{Confirm, MultiSelect, Select, theme::ColorfulTheme};
use gtk::gdk;

use crate::banner;
use crate::external::runtime_installer;
use crate::path_display::display_path;
use crate::setup::steps::{self, SetupAction};
use crate::setup::{common, config as setup_config, pipeline, sadx};
use crate::steam::game::{Game, GameKind};
use crate::steam::library::{self, DetectionResult};

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

    fn dim_writeln(&mut self, s: &str) -> std::io::Result<()> {
        if self.use_color {
            writeln!(self.writer, "{}", self.dim.apply_to(s))
        } else {
            writeln!(self.writer, "{s}")
        }
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
#[command(
    name = "adventure-mods",
    version = env!("CARGO_PKG_VERSION"),
    about = "The easiest way to mod Sonic Adventure DX and Sonic Adventure 2 on Linux",
    long_about = "The easiest way to mod Sonic Adventure DX and Sonic Adventure 2 on Linux. Finds your Steam installs, downloads community mods, and handles mod managers, runtimes, resolution, load order, and language settings so you can play right away. Run without a subcommand to launch the GUI."
)]
pub struct Cli {
    #[arg(long, help = "Disable colored CLI output")]
    no_color: bool,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(about = "Detect supported Steam installs and inaccessible Steam libraries")]
    Detect(DetectArgs),
    #[command(about = "List presets and recommended mods for a game")]
    ListMods {
        #[arg(long, help = "Game to inspect: sadx or sa2")]
        game: String,
    },
    #[command(about = "Run game setup in interactive or fully specified CLI mode")]
    Setup(SetupArgs),
}

#[derive(Debug, Args, Default)]
pub struct DetectArgs {
    #[arg(
        long,
        help = "Read Steam library data from a specific libraryfolders.vdf file"
    )]
    pub libraryfolders_vdf: Option<PathBuf>,
    #[arg(
        long = "steam-library",
        help = "Add an extra Steam library root to scan",
        verbatim_doc_comment
    )]
    pub steam_libraries: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct SetupArgs {
    #[arg(long, help = "Game to set up: sadx or sa2")]
    pub game: Option<String>,
    #[arg(long, help = "Comma-separated mod slugs to install")]
    pub mods: Option<String>,
    #[arg(long, help = "Named preset to install (SADX only)")]
    pub preset: Option<String>,
    #[arg(long, help = "Install all recommended mods for the selected game")]
    pub all_mods: bool,
    #[arg(
        long,
        help = "Subtitle language. SADX: english, japanese, french, spanish, german. SA2: english, german, spanish, french, italian, japanese"
    )]
    pub subtitle_language: Option<String>,
    #[arg(long, help = "Voice language: japanese or english")]
    pub voice_language: Option<String>,
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..), help = "Override generated render width")]
    pub width: Option<u32>,
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..), help = "Override generated render height")]
    pub height: Option<u32>,
    #[arg(
        long,
        help = "Override Steam detection with an explicit game install path"
    )]
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
    banner::print_header(&mut out.writer, env!("CARGO_PKG_VERSION"), out.use_color)?;

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
                out.path(&display_path(&game.path))
            ))?;
        }
    }

    if !result.inaccessible.is_empty() {
        out.heading("Inaccessible Steam libraries:")?;
        for game in &result.inaccessible {
            out.writeln(&format!(
                "- {}: {}",
                game.kind.name(),
                out.path(&display_path(&game.library_path))
            ))?;
        }
    }

    Ok(())
}

fn run_list_mods(game: &str, out: &mut CliOutput) -> Result<()> {
    let game_kind = parse_game_kind(game)?;
    banner::print_header(&mut out.writer, env!("CARGO_PKG_VERSION"), out.use_color)?;
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
            &format!("{} [{}]", mod_entry.name, mod_entry.slug),
            mod_entry.description,
        )?;
    }

    Ok(())
}

fn run_setup(args: SetupArgs, out: &mut CliOutput) -> Result<()> {
    let term_width = if out.use_color {
        console::Term::stdout().size().1 as usize
    } else {
        0
    };
    if term_width >= 30 {
        banner::print_banner(&mut out.writer, out.use_color)?;
    }
    banner::print_header(&mut out.writer, env!("CARGO_PKG_VERSION"), out.use_color)?;
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
    let (width, height) = match (args.width, args.height) {
        (Some(w), Some(h)) => (w, h),
        (w, h) => {
            let (dw, dh) = detect_resolution();
            (w.unwrap_or(dw), h.unwrap_or(dh))
        }
    };
    let language_selection = resolve_setup_languages(
        &args,
        game_kind,
        rich_prompts.then_some(&prompt as &dyn Prompt),
    )?;

    if rich_prompts {
        let summary = SetupSummary {
            game_kind,
            game_path: &game_path,
            selected_mods: &selected_mods,
            width,
            height,
            language_selection,
        };
        confirm_setup_summary(out, &summary, &prompt)?;
    }

    out.writeln(&format!(
        "Setting up {} at {}",
        game_kind.name(),
        out.path(&display_path(&game_path))
    ))?;
    out.writeln("")?;

    let mod_install = ModInstallStep {
        game_path: &game_path,
        game_kind,
        selected_mods: &selected_mods,
        width,
        height,
        language_selection,
    };
    let actions = steps::actions_for_game(game_kind);
    let total_steps = actions.len();

    for (offset, action) in actions.into_iter().enumerate() {
        let step_index = offset + 1;
        let label = action.cli_title();
        match action {
            SetupAction::InstallDotnet => {
                run_setup_step(out, step_index, total_steps, label, || {
                    runtime_installer::install_runtimes(&game_path, game_kind.app_id())
                })?;
            }
            SetupAction::ConvertSteam => {
                run_download_step(out, step_index, total_steps, label, |progress_fn| {
                    sadx::convert_steam_to_2004(&game_path, progress_fn)
                })?;
            }
            SetupAction::InstallModManager => {
                run_download_step(out, step_index, total_steps, label, |progress_fn| {
                    common::install_mod_manager(&game_path, game_kind, progress_fn)
                })?;
            }
            SetupAction::InstallMods => {
                run_mod_install_step(out, step_index, total_steps, label, &mod_install)?;
            }
        }
    }
    persist_cli_language_selection(game_kind, language_selection);

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

fn run_download_step<T>(
    out: &mut CliOutput,
    index: usize,
    total: usize,
    label: &str,
    action: impl FnOnce(Option<crate::external::download::ProgressFn>) -> Result<T>,
) -> Result<T> {
    let heading = step_heading(index, total, label);
    out.heading(&heading)?;

    // Track last printed MB to avoid flooding stdout
    let last_mb = std::sync::Arc::new(std::sync::Mutex::new(-1i64));
    let last_mb_clone = last_mb.clone();
    let wrote_progress = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let wrote_progress_clone = wrote_progress.clone();

    let use_color = out.use_color;
    let dim = out.dim.clone();
    let interactive_stderr = std::io::stderr().is_terminal();

    let progress_fn: Option<crate::external::download::ProgressFn> =
        Some(Box::new(move |downloaded, total_bytes| {
            let mb = (downloaded / 1_048_576) as i64;
            let mut last = last_mb_clone.lock().unwrap();
            if mb != *last {
                *last = mb;
                wrote_progress_clone.store(true, std::sync::atomic::Ordering::Relaxed);
                let text = if let Some(tb) = total_bytes {
                    let total_mb = tb as f64 / 1_048_576.0;
                    let pct = downloaded as f64 / tb as f64 * 100.0;
                    format!(
                        "  {:.1} / {:.1} MB ({:.0}%)",
                        downloaded as f64 / 1_048_576.0,
                        total_mb,
                        pct,
                    )
                } else {
                    format!("  {:.1} MB downloaded", downloaded as f64 / 1_048_576.0)
                };
                if interactive_stderr {
                    if use_color {
                        eprint!("\r{}", dim.apply_to(&text))
                    } else {
                        eprint!("\r{text}")
                    }
                } else {
                    if use_color {
                        eprintln!("{}", dim.apply_to(&text))
                    } else {
                        eprintln!("{text}")
                    }
                }
            }
        }));

    let value = action(progress_fn);
    if interactive_stderr && wrote_progress.load(std::sync::atomic::Ordering::Relaxed) {
        eprintln!();
    }
    let value = value?;
    out.writeln("Done")?;
    out.writeln("")?;
    Ok(value)
}

struct ModInstallStep<'a> {
    game_path: &'a std::path::Path,
    game_kind: GameKind,
    selected_mods: &'a [&'a common::ModEntry],
    width: u32,
    height: u32,
    language_selection: setup_config::LanguageSelection,
}

struct SetupSummary<'a> {
    game_kind: GameKind,
    game_path: &'a std::path::Path,
    selected_mods: &'a [&'a common::ModEntry],
    width: u32,
    height: u32,
    language_selection: setup_config::LanguageSelection,
}

fn run_mod_install_step(
    out: &mut CliOutput,
    index: usize,
    total: usize,
    label: &str,
    step: &ModInstallStep<'_>,
) -> Result<()> {
    out.heading(&step_heading(index, total, label))?;
    // Track last printed MB per mod name to avoid flooding stderr under concurrent downloads.
    let mut last_dl_mb_per_mod: std::collections::HashMap<String, i64> =
        std::collections::HashMap::new();
    let mut has_active_progress_line = false;
    let interactive_stderr = std::io::stderr().is_terminal();
    let result = pipeline::install_selected_mods_and_generate_config_with_progress(
        step.game_path,
        step.game_kind,
        step.selected_mods,
        step.width,
        step.height,
        step.language_selection,
        |progress| {
            match progress {
                pipeline::InstallProgress::Started { mod_name } => {
                    if interactive_stderr && has_active_progress_line {
                        eprintln!();
                        has_active_progress_line = false;
                    }
                    last_dl_mb_per_mod.insert(mod_name.to_string(), -1);
                    let _ = out.dim_writeln(&format!("  Starting: {mod_name}"));
                }
                pipeline::InstallProgress::DownloadingMod {
                    mod_name,
                    downloaded,
                    total_bytes,
                } => {
                    let mb = (downloaded / 1_048_576) as i64;
                    let last = last_dl_mb_per_mod.entry(mod_name.to_string()).or_insert(-1);
                    if mb != *last {
                        *last = mb;
                        let text = if let Some(tb) = total_bytes {
                            let pct = downloaded as f64 / tb as f64 * 100.0;
                            format!(
                                "    {mod_name}: {:.1} / {:.1} MB ({:.0}%)",
                                downloaded as f64 / 1_048_576.0,
                                tb as f64 / 1_048_576.0,
                                pct,
                            )
                        } else {
                            format!("    {mod_name}: {:.1} MB", downloaded as f64 / 1_048_576.0)
                        };
                        if interactive_stderr {
                            has_active_progress_line = true;
                            if out.use_color {
                                eprint!("\r{}", out.dim.apply_to(&text))
                            } else {
                                eprint!("\r{text}")
                            }
                        } else {
                            if out.use_color {
                                eprintln!("{}", out.dim.apply_to(&text))
                            } else {
                                eprintln!("{text}")
                            }
                        }
                    }
                }
                pipeline::InstallProgress::Finished {
                    mod_name,
                    completed,
                    total,
                } => {
                    if interactive_stderr && has_active_progress_line {
                        eprintln!();
                        has_active_progress_line = false;
                    }
                    last_dl_mb_per_mod.remove(mod_name);
                    let _ = out.writeln(&format!("  [{completed}/{total}] Installed: {mod_name}"));
                }
                pipeline::InstallProgress::GeneratingConfig => {
                    if interactive_stderr && has_active_progress_line {
                        eprintln!();
                        has_active_progress_line = false;
                    }
                    let _ = out.writeln("  Generating mod config...");
                }
            }
            Ok(())
        },
    );
    if interactive_stderr && has_active_progress_line {
        eprintln!();
    }
    result?;
    out.writeln("Done")?;
    out.writeln("")?;
    Ok(())
}

fn resolve_setup_languages(
    args: &SetupArgs,
    game_kind: GameKind,
    prompt: Option<&dyn Prompt>,
) -> Result<setup_config::LanguageSelection> {
    let mut selection =
        setup_config::load_language_selection(setup_config::app_settings().as_ref(), game_kind);

    if let Some(value) = &args.subtitle_language {
        let subtitle = setup_config::SubtitleLanguage::parse(value)?;
        if !setup_config::SubtitleLanguage::supported_for(game_kind).contains(&subtitle) {
            bail!(
                "Subtitle language '{}' is not supported for {}.",
                value,
                game_kind.name()
            );
        }
        selection.subtitle = subtitle;
    } else if let Some(p) = prompt {
        let options = setup_config::SubtitleLanguage::supported_for(game_kind);
        let labels: Vec<String> = options.iter().map(|l| l.label().to_string()).collect();
        let default_index = options
            .iter()
            .position(|&l| l == selection.subtitle)
            .unwrap_or(0);
        let selected = p.select("Subtitle language", &labels, default_index)?;
        selection.subtitle = options[selected];
    }

    if let Some(value) = &args.voice_language {
        selection.voice = setup_config::VoiceLanguage::parse(value)?;
    } else if let Some(p) = prompt {
        let options = setup_config::VoiceLanguage::all();
        let labels: Vec<String> = options.iter().map(|l| l.label().to_string()).collect();
        let default_index = options
            .iter()
            .position(|&l| l == selection.voice)
            .unwrap_or(0);
        let selected = p.select("Voice language", &labels, default_index)?;
        selection.voice = options[selected];
    }

    Ok(selection)
}

fn persist_cli_language_selection(
    game_kind: GameKind,
    language_selection: setup_config::LanguageSelection,
) {
    setup_config::save_language_selection(
        setup_config::app_settings().as_ref(),
        game_kind,
        language_selection,
    );
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
        .map(|game| format!("{} ({})", game.kind.name(), display_path(&game.path)))
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
            let items: Vec<String> = games.iter().map(|game| display_path(&game.path)).collect();
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
        bail!("--mods requires at least one mod slug");
    }

    Ok(parsed)
}

fn resolve_mod_identifier(
    game_kind: GameKind,
    identifier: &str,
) -> Result<&'static common::ModEntry> {
    common::recommended_mods_for_game(game_kind)
        .iter()
        .find(|mod_entry| mod_entry.slug.eq_ignore_ascii_case(identifier))
        .ok_or_else(|| {
            anyhow!(
                "Unknown mod slug '{}'. Use 'list-mods --game {}' to see valid slugs.",
                identifier,
                game_kind_arg(game_kind)
            )
        })
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
    let all_recommended_index = if presets.is_empty() {
        let idx = options.len();
        options.push("Install all recommended mods".to_string());
        Some(idx)
    } else {
        None
    };
    options.push("Choose mods manually".to_string());

    let selection = prompt.select("Choose setup mode", &options, 0)?;

    if selection < presets.len() {
        return pipeline::resolve_selected_mods(game_kind, Some(presets[selection].name), &[]);
    }
    if Some(selection) == all_recommended_index {
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
    summary: &SetupSummary<'_>,
    prompt: &dyn Prompt,
) -> Result<()> {
    out.heading("Summary")?;
    out.writeln(&format!("Game: {}", summary.game_kind.name()))?;
    out.writeln(&format!(
        "Path: {}",
        out.path(&display_path(summary.game_path))
    ))?;
    out.writeln(&format!("Resolution: {}x{}", summary.width, summary.height))?;
    out.writeln(&format!(
        "Subtitle language: {}",
        summary.language_selection.subtitle.label()
    ))?;
    out.writeln(&format!(
        "Voice language: {}",
        summary.language_selection.voice.label()
    ))?;
    out.writeln(&format!("Mods selected: {}", summary.selected_mods.len()))?;
    for mod_entry in summary.selected_mods {
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
        "No mod selection specified. Pass --all-mods, --preset <name>, or --mods <slug1,slug2>.\n\
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
        if arg == "--no-color" {
            index += 1;
            continue;
        }

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
            _ => {
                return Err(anyhow!(
                    console::strip_ansi_codes(&error.to_string())
                        .trim_end()
                        .to_string()
                ));
            }
        },
    };

    let use_color = !cli.no_color && std::env::var("NO_COLOR").is_err() && is_terminal;

    initialize_runtime()?;

    run_with_io(cli, use_color, output)?;
    Ok(true)
}

fn detect_resolution() -> (u32, u32) {
    let fallback = (1920u32, 1080u32);

    if let Some(res) = detect_resolution_via_gdk() {
        return res;
    }

    let output = std::process::Command::new("xrandr")
        .arg("--current")
        .output();

    let output = match output {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            tracing::warn!("xrandr exited with status {}", o.status);
            return fallback;
        }
        Err(e) => {
            tracing::warn!("Could not run xrandr: {e}");
            return fallback;
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);

    match parse_xrandr_resolution(&stdout) {
        Some((w, h)) => {
            tracing::info!("Detected resolution via xrandr: {w}x{h}");
            (w, h)
        }
        None => {
            tracing::warn!(
                "Could not detect monitor resolution via xrandr, using fallback {}x{}",
                fallback.0,
                fallback.1
            );
            fallback
        }
    }
}

fn detect_resolution_via_gdk() -> Option<(u32, u32)> {
    use std::sync::OnceLock;
    static GTK_INIT: OnceLock<bool> = OnceLock::new();

    // gtk::init() panics if called from a non-main thread. Use OnceLock so only
    // the first caller attempts it and any panic is caught rather than propagated.
    let initialized = GTK_INIT.get_or_init(|| std::panic::catch_unwind(gtk::init).is_ok());
    if !*initialized {
        return None;
    }
    let display = gdk::Display::default()?;
    crate::display::resolution_from_display(&display, None)
}

fn parse_xrandr_resolution(output: &str) -> Option<(u32, u32)> {
    let mut first_connected: Option<(u32, u32)> = None;

    for line in output.lines() {
        let is_connected = line.contains(" connected ") || line.ends_with(" connected");
        if !is_connected {
            continue;
        }

        let res = line.split_whitespace().find_map(|token| {
            let (dims, _offsets) = token.split_once('+')?;
            let (w_str, h_str) = dims.split_once('x')?;
            let w = w_str.parse::<u32>().ok()?;
            let h = h_str.parse::<u32>().ok()?;
            Some((w, h))
        });

        if let Some(res) = res {
            if line.contains(" primary ") {
                return Some(res);
            }
            if first_connected.is_none() {
                first_connected = Some(res);
            }
        }
    }

    first_connected
}

#[cfg(test)]
#[path = "cli_tests.rs"]
mod tests;
