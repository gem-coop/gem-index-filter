use facet::filter_versions_bytes;
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

    // Filter
    eprintln!("Filtering versions file...");
    let filtered = filter_versions_bytes(input, &allowlist)?;
    eprintln!("Filtered result: {} bytes", filtered.len());

    // Write output
    if let Some(output_path) = output_file {
        std::fs::write(output_path, &filtered)?;
        eprintln!("Written to {}", output_path);
    } else {
        io::Write::write_all(&mut io::stdout(), &filtered)?;
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
        if !gem_name.is_empty() && !gem_name.starts_with('#') {
            allowlist.insert(gem_name.to_string());
        }
    }

    Ok(allowlist)
}
