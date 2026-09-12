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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn print_banner_writes_plain_art_when_color_is_disabled() {
        let mut output = Vec::new();

        print_banner(&mut output, false).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(!output.is_empty());
        assert!(!output.contains('\x1b'));
    }

    #[test]
    fn print_banner_formats_each_line_when_color_is_enabled() {
        let mut output = Vec::new();

        print_banner(&mut output, true).unwrap();

        let output = String::from_utf8(output).unwrap();
        assert!(!output.is_empty());
    }

    #[test]
    fn print_header_supports_colored_and_plain_output() {
        let mut plain = Vec::new();
        print_header(&mut plain, "0.0.0-test", false).unwrap();
        assert_eq!(
            String::from_utf8(plain).unwrap(),
            "Adventure Mods v0.0.0-test\n"
        );

        let mut colored = Vec::new();
        print_header(&mut colored, "0.0.0-test", true).unwrap();
        assert!(
            String::from_utf8(colored)
                .unwrap()
                .contains("Adventure Mods")
        );
    }

    #[test]
    fn print_header_propagates_writer_errors() {
        struct FailingWriter;

        impl Write for FailingWriter {
            fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("synthetic writer failure"))
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let error = print_header(&mut FailingWriter, "1.2.3", true).unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::Other);
        FailingWriter.flush().unwrap();
    }
}
