use anyhow::{bail, Context, Result};
use clap::Parser;
use data_encoding::HEXUPPER;
use image::ImageFormat;
use rayon::prelude::*;
use ring::digest::{Context as DigestContext, SHA256};
use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

mod game_extractor;
mod img;
mod network;

use game_extractor::{GameExtractor, JonssonDjupet, JonssonMjolner};
use img::process_image;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input directory containing the disc contents
    #[arg(short, long, default_value = "disc_contents")]
    input_dir: String,

    /// Output directory for processed files
    #[arg(short, long, default_value = "output")]
    output_dir: String,

    /// Disable WebP compression (default: compression enabled)
    #[arg(long)]
    no_compression: bool,

    /// Disable upscaling (default: upscaling enabled)
    #[arg(long)]
    no_upscale: bool,
}

pub fn detect_game(input_dir: &Path) -> Result<Box<dyn GameExtractor>> {
    let dir_files = find_dir_files(input_dir)?;

    let jonsson_mjolner_files: HashSet<String> = [
        "anslagstavla.dir",
        "block.dir",
        "dorislapp.dir",
        "glidflygare.dir",
        "heden.dir",
        "kassaskap.dir",
        "monalisa.dir",
        "paris.dir",
        "setup.dir",
        "souvenir.dir",
        "tavla.dir",
        "tidningsbutik.dir",
        "wtavla.dir",
        "berlin.dir",
        "container.dir",
        "drottningtavla.dir",
        "gotland.dir",
        "huvudmeny.dir",
        "london.dir",
        "nrspel.dir",
        "rom.dir",
        "sheild.dir",
        "stockholm.dir",
        "telefonbok.dir",
        "wsafe.dir",
    ]
    .iter()
    .map(|&s| s.to_lowercase())
    .collect();

    let jonsson_djupet_files: HashSet<String> = ["avi.dir", "game.dir", "mainmenu.dir", "qt.dir"]
        .iter()
        .map(|&s| s.to_lowercase())
        .collect();

    let found_files: HashSet<String> = dir_files
        .iter()
        .filter_map(|path| path.file_name())
        .filter_map(|name| name.to_str())
        .map(|s| s.to_lowercase())
        .collect();

    let game_a_match = jonsson_mjolner_files.intersection(&found_files).count();
    let game_b_match = jonsson_djupet_files.intersection(&found_files).count();

    if game_a_match > game_b_match {
        if game_a_match < jonsson_mjolner_files.len() {
            println!("Warning: Only found {} out of {} expected files for Jönssonligan: Jakten på Mjölner. Proceeding anyway.", game_a_match, jonsson_mjolner_files.len());
        }
        Ok(Box::new(JonssonMjolner))
    } else if game_b_match > 0 {
        if game_b_match < jonsson_djupet_files.len() {
            println!(
                "Warning: Only found {} out of {} expected files for Jönssonligan: Går på Djupet. Proceeding anyway.", game_b_match, jonsson_djupet_files.len()
            );
        }
        Ok(Box::new(JonssonDjupet))
    } else {
        anyhow::bail!("Unable to detect game type. No matching .dir files found.")
    }
}

fn find_dir_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut dir_files = Vec::new();
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                dir_files.extend(find_dir_files(&path)?);
            } else if path
                .extension()
                .map_or(false, |ext| ext.eq_ignore_ascii_case("dir"))
            {
                dir_files.push(path);
            }
        }
    }
    Ok(dir_files)
}

#[cfg(target_os = "windows")]
fn run_extractor(temp_dir: &Path, dir_file: &str) -> Result<std::process::Output> {
    Command::new(temp_dir.join("dir_extractor.exe"))
        .arg(dir_file)
        .current_dir(temp_dir)
        .output()
        .context("Failed to run extractor on Windows")
}

#[cfg(not(target_os = "windows"))]
fn run_extractor(temp_dir: &Path, dir_file: &str) -> Result<std::process::Output> {
    Command::new("wine")
        .arg("dir_extractor.exe")
        .arg(dir_file)
        .current_dir(temp_dir)
        .output()
        .context("Failed to run extractor with Wine")
}

#[cfg(not(target_os = "windows"))]
fn check_wine_installation() -> Result<()> {
    let output = Command::new("wine")
        .arg("--version")
        .output()
        .context("Failed to execute 'wine --version'. Is Wine installed and in your PATH?")?;

    if !output.status.success() {
        bail!("Wine is not properly installed or configured. Please install Wine and ensure it's in your PATH.");
    }

    Ok(())
}

fn extract_files(temp_dir: &Path) -> Result<()> {
    let dir_files = find_files(temp_dir, ".dir")
        .context("Failed to find .dir files. Make sure the input directory is correct and contains .dir files.")?;
    if dir_files.is_empty() {
        bail!("No .dir files found in the input directory. Please check your input path.");
    }
    let total = dir_files.len();

    for (i, dir) in dir_files.iter().enumerate() {
        let file_name = dir.file_name().to_string_lossy().into_owned();
        println!(
            "Extracting assets from: {:?} ({}/{})",
            file_name,
            i + 1,
            total
        );
        run_extractor(temp_dir, &file_name)
            .context(format!("Failed to extract assets from: {:?}", file_name))?;
    }

    Ok(())
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).context("Failed to open file for hashing")?;
    let mut context = DigestContext::new(&SHA256);
    let mut buffer = [0; 8192];

    loop {
        let count = file.read(&mut buffer).context("Failed to read file")?;
        if count == 0 {
            break;
        }
        context.update(&buffer[..count]);
    }

    let digest = context.finish();
    Ok(HEXUPPER.encode(digest.as_ref()))
}

fn remove_duplicates(path: &Path) -> Result<()> {
    let mut set = HashSet::new();
    let mut files: Vec<PathBuf> = fs::read_dir(path)
        .context("Failed to read output directory")?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();

    files.sort();

    for path in files {
        let hash = hash_file(&path)?;
        if set.contains(&hash) {
            fs::remove_file(&path).context("Failed to remove duplicate file")?;
        } else {
            set.insert(hash);
        }
    }
    Ok(())
}

fn find_files(dir: &Path, extension: &str) -> Result<Vec<fs::DirEntry>> {
    let mut files: Vec<fs::DirEntry> = fs::read_dir(dir)
        .context("Failed to read directory")?
        .filter_map(|res| res.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .map_or(false, |ext| ext == extension.trim_start_matches('.'))
        })
        .collect();

    files.sort_by_key(|dir| dir.path());
    Ok(files)
}

fn move_file_to_output(src_path: &Path, output_dir: &Path, extension: Option<&str>) -> Result<()> {
    let file_name = src_path
        .file_name()
        .and_then(|os_str| os_str.to_str())
        .context("Invalid file name")?;

    let parts: Vec<&str> = file_name.split("--").collect();
    let mut dst_path = output_dir.to_path_buf();

    if parts.len() > 1 {
        dst_path.extend(&parts[..parts.len() - 1]);

        let file_parts: Vec<&str> = parts.last().unwrap().split("__").collect();
        if file_parts.len() > 1 {
            dst_path.push(&file_parts[0]);
            let mut final_name = file_parts[1..].join("__");
            if final_name.starts_with('-') {
                final_name = final_name[1..].to_string();
            }
            dst_path.push(final_name);
        } else {
            dst_path.push(parts.last().unwrap());
        }
    } else {
        dst_path.push(file_name);
    }

    if let Some(ext) = extension {
        dst_path.set_extension(ext);
    }

    fs::create_dir_all(dst_path.parent().unwrap())?;
    fs::rename(src_path, &dst_path)
        .or_else(|_| fs::copy(src_path, &dst_path).map(|_| ()))
        .with_context(|| format!("Failed to move file: {:?}", src_path))?;

    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    let input_dir = Path::new(&args.input_dir);
    let output_dir = Path::new(&args.output_dir);
    let extractor_tools_dir = Path::new("extractor_tools");

    let game = detect_game(&input_dir)?;

    println!("Found {} assets. Starting extraction.", game.get_name());

    if !input_dir.exists() {
        bail!(
            "Input directory '{}' does not exist. Please check your input path.",
            input_dir.display()
        );
    }

    let input_dir = fs::read_dir(&input_dir)
        .context("Failed to read input directory")?
        .filter_map(Result::ok)
        .find(|entry| {
            let path = entry.path();
            path.is_dir()
                && path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_lowercase() == "data")
                    .unwrap_or(false)
        })
        .map(|entry| entry.path())
        .context("No data or Data directory found")?;

    #[cfg(not(target_os = "windows"))]
    check_wine_installation()?;

    let temp_dir = env::temp_dir().join(format!("cgex_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).context("Failed to create temporary directory")?;

    game_extractor::copy_directory(&input_dir, &temp_dir)
        .context("Failed to copy input directory to temp")?;

    game.prepare_temp_directory(&temp_dir)?;

    fs::copy(
        extractor_tools_dir.join("dir_extractor.exe"),
        temp_dir.join("dir_extractor.exe"),
    )
    .context("Failed to copy dir_extractor.exe")?;

    let xtras_src = extractor_tools_dir.join("Xtras");
    let xtras_dst = temp_dir.join("Xtras");
    game_extractor::copy_directory(&xtras_src, &xtras_dst)
        .context("Failed to copy Xtras folder")?;

    extract_files(&temp_dir).context("Failed to extract files")?;

    println!("Removing duplicates. This might take a while...");
    if let Err(e) = remove_duplicates(&temp_dir) {
        println!("Warning: Failed to remove duplicate files: {}", e);
        println!("Continuing with processing...");
    }

    let broken_images = game.get_broken_images();
    for file in &broken_images {
        let path = temp_dir.join(file);
        if let Err(e) = fs::remove_file(&path) {
            println!("Warning: Failed to remove file {:?}: {}", path, e);
        }
    }

    println!(
        "Processing images{}{}. This might take a while...",
        if args.no_upscale {
            ""
        } else {
            " with AI-upscaling"
        },
        if args.no_compression {
            ""
        } else {
            " and compression"
        }
    );

    let bmp_files =
        find_files(&temp_dir, ".bmp").context("Failed to find BMP files for processing")?;
    let total = bmp_files.len();
    let counter = AtomicUsize::new(1);

    let processed_files: Vec<(PathBuf, ImageFormat)> = bmp_files
        .into_par_iter()
        .map(|entry| -> Result<(PathBuf, ImageFormat)> {
            let current = counter.fetch_add(1, Ordering::SeqCst);
            println!(
                "Processing: {:?} ({}/{})",
                entry.file_name(),
                current,
                total
            );

            let input_path = entry.path();
            let output_path = temp_dir.join(input_path.file_name().unwrap());
            let format = process_image(
                &input_path,
                &output_path,
                !args.no_compression,
                !args.no_upscale,
                game.get_transparent_color(),
            )
            .context(format!("Failed to process image: {:?}", input_path))?;
            Ok((output_path, format))
        })
        .collect::<Result<Vec<_>>>()?;

    game.post_extraction_setup(&temp_dir, &processed_files)?;

    println!("Moving files into final directory structure");
    fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    for (temp_path, format) in processed_files {
        let extension = format.extensions_str()[0];
        move_file_to_output(&temp_path, output_dir, Some(extension))
            .context(format!("Failed to move processed file: {:?}", temp_path))?;
    }

    let wav_files = find_files(&temp_dir, ".wav").context("Failed to find WAV files for moving")?;
    for file in wav_files {
        let src_path = file.path();
        move_file_to_output(&src_path, output_dir, None)
            .context(format!("Failed to move WAV file: {:?}", src_path))?;
    }

    println!("Cleaning up temporary directory");
    fs::remove_dir_all(&temp_dir).context("Failed to remove temporary directory")?;

    println!("Processing complete!");
    Ok(())
}
