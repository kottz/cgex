use anyhow::{bail, Context, Result};
use clap::Parser;
use data_encoding::HEXUPPER;
use rayon::prelude::*;
use ring::digest::{Context as DigestContext, SHA256};
use std::collections::HashSet;
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

mod img;
mod network;

use img::process_image;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Input directory containing the disc contents
    #[arg(short, long, default_value = "disc_contents/data")]
    input_dir: String,

    /// Output directory for processed files
    #[arg(short, long, default_value = "output")]
    output_dir: String,
}

fn copy_directory(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_directory(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
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

    let mut counted_files = fs::read_dir(temp_dir).iter().count();
    for (i, dir) in dir_files.iter().enumerate() {
        let file_name = dir.file_name().to_string_lossy().into_owned();
        print!(
            "Extracting assets from: {:?} ({}/{})",
            file_name,
            i + 1,
            total
        );
        run_extractor(temp_dir, &file_name)
            .context(format!("Failed to extract assets from: {:?}", file_name))?;
        // count the number of files in the directory now
        let tot_files = fs::read_dir(temp_dir).iter().count();
        println!(
            " - Files extracted from archive: {}",
            tot_files - counted_files
        );
        counted_files = tot_files;
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
    let output_files = fs::read_dir(path).context("Failed to read output directory")?;

    for entry in output_files {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();

        if path.is_file() {
            let hash = hash_file(&path)?;

            if set.contains(&hash) {
                fs::remove_file(&path).context("Failed to remove duplicate file")?;
            } else {
                set.insert(hash);
            }
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

fn copy_and_remove(src: &Path, dst: &Path) -> Result<()> {
    if let Some(parent) = dst.parent() {
        fs::create_dir_all(parent).context(format!("Failed to create directory {:?}", parent))?;
    }
    fs::copy(src, dst).context(format!("Failed to copy file from {:?} to {:?}", src, dst))?;
    fs::remove_file(src).context(format!(
        "Failed to remove original file {:?} after copying",
        src
    ))?;
    Ok(())
}

fn main() -> Result<()> {
    let args = Args::parse();

    let input_dir = Path::new(&args.input_dir);
    let output_dir = Path::new(&args.output_dir);
    let extractor_tools_dir = Path::new("extractor_tools");

    if !input_dir.exists() {
        bail!(
            "Input directory '{}' does not exist. Please check your input path.",
            input_dir.display()
        );
    }

    #[cfg(not(target_os = "windows"))]
    check_wine_installation()?;

    // Create a unique temporary directory
    let temp_dir = env::temp_dir().join(format!("game_extractor_{}", std::process::id()));
    fs::create_dir_all(&temp_dir).context("Failed to create temporary directory")?;
    println!("Using temporary directory: {}", temp_dir.display());

    println!("Copying files to temporary directory");
    copy_directory(input_dir, &temp_dir).context("Failed to copy input directory to temp")?;

    // Copy dir_extractor.exe
    fs::copy(
        extractor_tools_dir.join("dir_extractor.exe"),
        temp_dir.join("dir_extractor.exe"),
    )
    .context("Failed to copy dir_extractor.exe")?;

    // Copy and overwrite Xtras folder
    let xtras_src = extractor_tools_dir.join("Xtras");
    let xtras_dst = temp_dir.join("Xtras");
    copy_directory(&xtras_src, &xtras_dst).context("Failed to copy Xtras folder")?;

    println!("Extracting assets");
    extract_files(&temp_dir).context("Failed to extract files")?;

    println!("Removing duplicates. This might take a while...");
    if let Err(e) = remove_duplicates(&temp_dir) {
        println!("Warning: Failed to remove duplicate files: {}", e);
        println!("Continuing with processing...");
    }

    let broken_images = [
        "berlin--Animationer__harry0000-166.bmp",
        "berlin--Animationer__ingo0000-80.bmp",
        "berlin--Animationer__ingo0041-121.bmp",
        "berlin--Animationer__ingo0042-122.bmp",
        "berlin--Animationer__sickan0000-37.bmp",
        "berlin--Animationer__sickan0001-38.bmp",
        "berlin--Animationer__sickan0042.bmp",
        "berlin--Animationer__vanheden0000-123.bmp",
        "berlin--Animationer__vanheden0042-165.bmp",
    ];

    for file in &broken_images {
        let path = temp_dir.join(file);
        println!("Removing: {:?}", path);
        if let Err(e) = fs::remove_file(&path) {
            println!("Warning: Failed to remove file {:?}: {}", path, e);
        }
    }

    println!("Performing AI-upscaling. This might take a while...");
    let bmp_files =
        find_files(&temp_dir, ".bmp").context("Failed to find BMP files for upscaling")?;
    let total = bmp_files.len();
    let counter = AtomicUsize::new(1);

    bmp_files.par_iter().try_for_each(|entry| -> Result<()> {
        let current = counter.fetch_add(1, Ordering::SeqCst);
        println!(
            "Performing upscaling on: {:?} ({}/{})",
            entry.file_name(),
            current,
            total
        );

        let input_path = entry.path();
        let output_path = input_path.with_extension("png");

        process_image(&input_path, &output_path)
            .context(format!("Failed to process image: {:?}", input_path))?;
        Ok(())
    })?;

    println!("Moving files into final directory structure");
    fs::create_dir_all(output_dir).context("Failed to create output directory")?;
    for extension in &[".png", ".wav"] {
        let files = find_files(&temp_dir, extension)
            .context(format!("Failed to find {} files for moving", extension))?;
        for file in files {
            let src_path = file.path();
            let file_name = src_path.file_name().unwrap().to_str().unwrap();

            // Split by "--" for folders, then by "__" for the final filename
            let parts: Vec<&str> = file_name.split("--").collect();
            let dst_path = if parts.len() > 1 {
                let mut path = output_dir.to_path_buf();
                for part in &parts[..parts.len() - 1] {
                    path.push(part);
                }

                // Handle the last part (which may contain "__")
                let last_part = parts.last().unwrap();
                let file_parts: Vec<&str> = last_part.split("__").collect();
                if file_parts.len() > 1 {
                    path.push(file_parts[0]);
                    path.push(file_parts[1..].join("__"));
                } else {
                    path.push(last_part);
                }

                path
            } else {
                output_dir.join(file_name)
            };

            copy_and_remove(&src_path, &dst_path)
                .context(format!("Failed to move file: {:?}", src_path))?;
        }
    }

    println!("Cleaning up temporary directory");
    fs::remove_dir_all(&temp_dir).context("Failed to remove temporary directory")?;

    println!("Processing complete!");
    Ok(())
}
