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

use game_extractor::{GameExtractor, JonssonDjupet, JonssonMjolner, MulleBat, MulleBil};
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

    /// Enable WebP compression (default: output png)
    #[arg(long)]
    compression: bool,

    /// Disable upscaling (default: upscaling enabled)
    #[arg(long)]
    no_upscale: bool,

    /// Do not handle transparent background; leave background colors intact
    #[arg(long)]
    no_transparent_background: bool,
}

pub fn detect_game(input_dir: &Path) -> Result<Box<dyn GameExtractor>> {
    let dir_files = find_dir_files(input_dir)?;
    let found_files: HashSet<String> = dir_files
        .iter()
        .filter_map(|path| path.file_name())
        .filter_map(|name| name.to_str())
        .map(|s| s.to_lowercase())
        .collect();

    let games: Vec<Box<dyn GameExtractor>> = vec![
        Box::new(JonssonMjolner),
        Box::new(JonssonDjupet),
        Box::new(MulleBil),
        Box::new(MulleBat),
    ];

    let mut best_match: Option<(usize, Box<dyn GameExtractor>)> = None;

    for game in games {
        let expected_files = game.get_expected_files();
        let match_count = expected_files.intersection(&found_files).count();

        if match_count > 0 {
            if let Some((best_count, _)) = best_match {
                if match_count > best_count {
                    best_match = Some((match_count, game));
                }
            } else {
                best_match = Some((match_count, game));
            }
        }
    }

    if let Some((match_count, game)) = best_match {
        let expected_count = game.get_expected_files().len();
        if match_count < expected_count {
            println!(
                "Warning: Only found {} out of {} expected files for {}. Proceeding anyway.",
                match_count,
                expected_count,
                game.get_name()
            );
        }
        Ok(game)
    } else {
        bail!("Unable to detect game type. No matching .dir files found.")
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
            } else if path.extension().map_or(false, |ext| {
                ext.eq_ignore_ascii_case("dir") || ext.eq_ignore_ascii_case("dxr")
            }) {
                dir_files.push(path);
            }
        }
    }
    Ok(dir_files)
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

fn extract_files(temp_dir: &Path, game: &Box<dyn GameExtractor>) -> Result<()> {
    let files = find_files(temp_dir, &[".dir", ".dxr"])
        .context("Failed to find .dir or .dxr files. Make sure the input directory is correct and contains these files.")?;

    if files.is_empty() {
        bail!("No .dir or .dxr files found in the input directory. Please check your input path.");
    }

    let total = files.len();
    for (i, file) in files.iter().enumerate() {
        let file_name = file.file_name().to_string_lossy().into_owned();
        println!(
            "Extracting assets from: {:?} ({}/{})",
            file_name,
            i + 1,
            total
        );
        game.run_extractor(temp_dir, &file_name)
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

fn find_files(dir: &Path, extensions: &[&str]) -> Result<Vec<fs::DirEntry>> {
    let mut files: Vec<fs::DirEntry> = fs::read_dir(dir)
        .context("Failed to read directory")?
        .filter_map(|res| res.ok())
        .filter(|entry| {
            entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map_or(false, |ext| {
                    extensions.iter().any(|&valid_ext| {
                        ext.eq_ignore_ascii_case(valid_ext.trim_start_matches('.'))
                    })
                })
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

    #[cfg(not(target_os = "windows"))]
    check_wine_installation()?;

    let temp_dir = env::temp_dir().join(format!("cgex_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).context("Failed to create temporary directory")?;

    // Copy the entire input directory to temp
    game_extractor::copy_directory(&input_dir, &temp_dir)
        .context("Failed to copy input directory to temp")?;

    fs::copy(
        extractor_tools_dir.join("dir_extractor.exe"),
        temp_dir.join("dir_extractor.exe"),
    )
    .context("Failed to copy dir_extractor.exe")?;

    let xtras_src = extractor_tools_dir.join("Xtras");
    let xtras_dst = temp_dir.join("Xtras");
    game_extractor::copy_directory(&xtras_src, &xtras_dst)
        .context("Failed to copy Xtras folder")?;

    // Prepare the temp directory based on the specific game requirements
    game.prepare_temp_directory(&temp_dir)?;

    extract_files(&temp_dir, &game).context("Failed to extract files")?;

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
        if args.compression {
            " and compression"
        } else {
            ""
        }
    );

    let bmp_files =
        find_files(&temp_dir, &[".bmp"]).context("Failed to find BMP files for processing")?;
    let total = bmp_files.len();
    let counter = AtomicUsize::new(1);

    let processed_files: Vec<Result<(PathBuf, ImageFormat)>> = bmp_files
        .into_par_iter()
        .map(|entry| -> Result<(PathBuf, ImageFormat)> {
            let current = counter.fetch_add(1, Ordering::SeqCst);
            let file_n = entry.file_name();
            let file_name = file_n.to_string_lossy();
            println!("Processing: {:?} ({}/{})", file_name, current, total);

            let input_path = entry.path();
            let output_path = temp_dir.join(input_path.file_name().unwrap());
            process_image(
                &input_path,
                &output_path,
                args.compression,
                !args.no_upscale,
                game.get_transparent_color(),
                !args.no_transparent_background,
            )
            .map(|format| (output_path, format))
            .with_context(|| format!("Failed to process image: {:?}", input_path))
        })
        .collect();

    // Handle successful and failed image processing
    let (successful, failed): (Vec<_>, Vec<_>) =
        processed_files.into_iter().partition(Result::is_ok);

    let successful: Vec<(PathBuf, ImageFormat)> =
        successful.into_iter().map(Result::unwrap).collect();

    // Report failed images
    for error in failed {
        if let Err(e) = error {
            eprintln!("Error processing image: {}", e);
        }
    }

    game.post_extraction_setup(&temp_dir, &successful)?;

    println!("Moving files into final directory structure");
    fs::create_dir_all(output_dir).context("Failed to create output directory")?;

    for (temp_path, format) in successful {
        let extension = format.extensions_str()[0];
        move_file_to_output(&temp_path, output_dir, Some(extension))
            .with_context(|| format!("Failed to move processed file: {:?}", temp_path))?;
    }

    let wav_files =
        find_files(&temp_dir, &[".wav"]).context("Failed to find WAV files for moving")?;
    for file in wav_files {
        let src_path = file.path();
        move_file_to_output(&src_path, output_dir, None)
            .context(format!("Failed to move WAV file: {:?}", src_path))?;
    }

    let txt_files =
        find_files(&temp_dir, &[".txt"]).context("Failed to find WAV files for moving")?;
    for file in txt_files {
        let src_path = file.path();
        move_file_to_output(&src_path, output_dir, None)
            .context(format!("Failed to move txt file: {:?}", src_path))?;
    }
    println!("Cleaning up temporary directory");
    fs::remove_dir_all(&temp_dir).context("Failed to remove temporary directory")?;

    println!("Processing complete!");
    Ok(())
}
