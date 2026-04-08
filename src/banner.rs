use std::io::Write;

use console::Style;

const SONIC_BANNER: &str = include_str!("banner.txt");

pub fn print_banner(output: &mut impl Write, use_color: bool) -> std::io::Result<()> {
    if !use_color {
        write!(output, "{SONIC_BANNER}")?;
        return Ok(());
    }

    let cyan = Style::new().cyan().bright();
    for line in SONIC_BANNER.lines() {
        writeln!(output, "{}", cyan.apply_to(line))?;
    }
    Ok(())
}

pub fn print_header(
    output: &mut impl Write,
    version: &str,
    use_color: bool,
) -> std::io::Result<()> {
    if use_color {
        let bold = Style::new().bold();
        let yellow = Style::new().yellow().bright();
        writeln!(
            output,
            "{} {}",
            bold.apply_to("Adventure Mods"),
            yellow.apply_to(format!("v{version}"))
        )?;
    } else {
        writeln!(output, "Adventure Mods v{version}")?;
    }
    Ok(())
}
