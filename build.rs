fn main() {
    println!(
        "cargo:rustc-env=BUILD_TARGET={}",
        std::env::var("TARGET").unwrap()
    );

    for entry in std::fs::read_dir("localization").unwrap() {
        let path = entry.unwrap().path().join("messages.ftl");
        println!("cargo::rerun-if-changed={}", path.display(),);
        check_fluent_file(&path);
    }
}

fn check_fluent_file(path: &std::path::Path) {
    let file = match std::fs::read_to_string(path) {
        Ok(file) => file,
        Err(e) => {
            println!("cargo::error=failed to load {}: {e}", path.display());
            return;
        }
    };
    if let Err((_, errs)) = fluent_syntax::parser::parse(file.as_str()) {
        let pre = "cargo::error=";
        let red = "\x1b[91m";
        let blue = "\x1b[94m";
        let bold = "\x1b[1m";
        let reset_fg = "\x1b[39m";
        let reset = "\x1b[0m";
        for err in errs {
            println!("{pre}{red}{bold}error{reset_fg}: {}{reset}", err.kind);
            let line_number = &file[..err.pos.start].lines().count() + 1;
            println!("{pre} {blue}-->{reset} {}:{line_number}", path.display());
            if let Some(slice) = err.slice {
                let mut current_offset = slice.start;
                println!("{pre}  {bold}{blue}|{reset}");
                let mut has_last_line = false;
                for line in file[slice].trim().lines() {
                    has_last_line = false;
                    println!("{pre}  {bold}{blue}|{reset} {line}");
                    if err.pos.start < current_offset + line.len() && err.pos.end > current_offset {
                        let spaces = err.pos.start.saturating_sub(current_offset);
                        let arrows = (err.pos.end - current_offset).min(line.len()) - spaces;
                        println!(
                            "{pre}  {bold}{blue}|{reset} {:spaces$}{bold}{red}{:^<arrows$}{reset}",
                            "", ""
                        );
                        has_last_line = true;
                    }
                    current_offset += line.len() + 1;
                }
                if !has_last_line {
                    println!("{pre}  {bold}{blue}|{reset}");
                }
                println!("{pre}");
            }
        }
    }
}
