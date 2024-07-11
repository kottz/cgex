use anyhow::{bail, Context, Result};
use data_encoding::HEXUPPER;
use rayon::prelude::*;
use ring::digest::{Context as DigestContext, SHA256};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::Read;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

mod img;
mod network;

use img::process_image;

#[cfg(target_os = "windows")]
fn run_extractor(dir_path: &Path) -> Result<std::process::Output> {
    Command::new("extractor_tools/dir_extractor.exe")
        .current_dir("output")
        .arg(dir_path)
        .output()
        .context("Failed to run extractor on Windows")
}

#[cfg(not(target_os = "windows"))]
fn run_extractor(dir_path: &Path) -> Result<std::process::Output> {
    Command::new("wine")
        .current_dir("output")
        .arg("extractor_tools/dir_extractor.exe")
        .arg(dir_path)
        .output()
        .context("Failed to run extractor with Wine")
}

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

fn extract_files(input_dir: &Path, _output_dir: &Path) -> Result<()> {
    let dir_files = find_files(input_dir, ".dir")
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

        let dir_path = dir.path().canonicalize().context(format!(
            "Failed to canonicalize path for file: {:?}",
            file_name
        ))?;
        run_extractor(&dir_path)
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
    let output_files = fs::read_dir(path).context("Failed to read output directory")?;

    for entry in output_files {
        let entry = entry.context("Failed to read directory entry")?;
        let path = entry.path();
        let hash = hash_file(&path)?;

        if set.contains(&hash) {
            fs::remove_file(&path).context("Failed to remove duplicate file")?;
        } else {
            set.insert(hash);
        }
    }

    Ok(())
}

fn move_to_folder(dir: &fs::DirEntry, output_dir: &Path) -> Result<()> {
    let file_name = dir.file_name().to_string_lossy().replace("--", "/");
    let file_vec: Vec<&str> = file_name.split("__").collect();
    let dir_path = output_dir.join(&file_vec[0]);
    fs::create_dir_all(&dir_path).context("Failed to create directory")?;

    let new_path = output_dir.join(file_name.replace("__", "/"));
    fs::rename(dir.path(), new_path).context("Failed to move file")?;

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

fn main() -> Result<()> {
    let input_dir = Path::new("disc_contents/data");
    let output_dir = Path::new("output");

    if !input_dir.exists() {
        bail!(
            "Input directory '{}' does not exist. Please check your input path.",
            input_dir.display()
        );
    }

    fs::create_dir_all(output_dir).context(format!(
        "Failed to create output directory: {:?}",
        output_dir
    ))?;

    #[cfg(not(target_os = "windows"))]
    check_wine_installation()?;

    println!("Extracting assets from disc");
    extract_files(input_dir, output_dir).context("Failed to extract files from the disc")?;

    println!("Removing duplicates. This might take a while...");
    remove_duplicates(output_dir).context("Failed to remove duplicate files")?;

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
        let path = output_dir.join(file);
        println!("Removing: {:?}", path);
        if let Err(e) = fs::remove_file(&path) {
            println!("Warning: Failed to remove file {:?}: {}", path, e);
        }
    }

    println!("Performing AI-upscaling. This might take a while...");
    let bmp_files =
        find_files(output_dir, ".bmp").context("Failed to find BMP files for upscaling")?;
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
        let mut output_path = input_path.clone();
        output_path.set_extension("png");

        process_image(&input_path, &output_path)
            .context(format!("Failed to process image: {:?}", input_path))?;
        Ok(())
    })?;

    println!("Moving files into final directory structure");
    for extension in &[".png", ".wav"] {
        let files = find_files(output_dir, extension)
            .context(format!("Failed to find {} files for moving", extension))?;
        for file in files {
            move_to_folder(&file, output_dir)
                .context(format!("Failed to move file: {:?}", file.path()))?;
        }
    }

    println!("Removing unused BMP files");
    let bmp_files =
        find_files(output_dir, ".bmp").context("Failed to find BMP files for removal")?;
    for file in bmp_files {
        fs::remove_file(file.path())
            .context(format!("Failed to remove BMP file: {:?}", file.path()))?;
    }

    println!("Processing complete!");
    Ok(())
}
