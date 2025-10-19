use facet::filter::filter_versions_streaming;
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader};

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: facet <versions-file> <allowlist-file> [output-file]");
        eprintln!();
        eprintln!("Arguments:");
        eprintln!("  <versions-file>   Path to the versions file (or - for stdin)");
        eprintln!("  <allowlist-file>  Path to file with gem names (one per line)");
        eprintln!("  [output-file]     Optional output file (defaults to stdout)");
        eprintln!();
        eprintln!("Example:");
        eprintln!("  facet versions.txt allowlist.txt filtered.txt");
        eprintln!("  curl https://rubygems.org/versions | facet - allowlist.txt > filtered.txt");
        std::process::exit(1);
    }

    let versions_file = &args[1];
    let allowlist_file = &args[2];
    let output_file = args.get(3);

    // Read allowlist
    let allowlist_owned = read_allowlist(allowlist_file)?;
    eprintln!("Loaded {} gems from allowlist", allowlist_owned.len());

    // Convert to &str references for API
    let allowlist: HashSet<&str> = allowlist_owned.iter().map(|s| s.as_str()).collect();

    // Open input
    let input: Box<dyn io::Read> = if versions_file == "-" {
        Box::new(io::stdin())
    } else {
        Box::new(File::open(versions_file)?)
    };

    // Stream and filter directly to output
    eprintln!("Streaming and filtering versions file...");
    if let Some(output_path) = output_file {
        let mut output = File::create(output_path)?;
        filter_versions_streaming(input, &mut output, &allowlist)?;
        eprintln!("Written to {}", output_path);
    } else {
        let mut output = io::stdout();
        filter_versions_streaming(input, &mut output, &allowlist)?;
    }

    Ok(())
}

/// Read allowlist from file (one gem name per line)
fn read_allowlist(path: &str) -> io::Result<HashSet<String>> {
    let file = File::open(path)?;
    let reader = BufReader::new(file);
    let mut allowlist = HashSet::new();

    for line in reader.lines() {
        let line = line?;
        let gem_name = line.trim();
        if !gem_name.is_empty() {
            allowlist.insert(gem_name.to_string());
        }
    }

    Ok(allowlist)
}
