use anyhow::{Context, Result};
use image::ImageFormat;
use std::collections::HashSet;
use std::fs::{self};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub trait GameExtractor: Send + Sync {
    fn prepare_temp_directory(&self, temp_dir: &Path) -> Result<()>;
    fn get_transparent_color(&self) -> [u8; 3];
    fn post_extraction_setup(
        &self,
        temp_dir: &Path,
        processed_files: &[(PathBuf, ImageFormat)],
    ) -> Result<()>;
    fn get_broken_images(&self) -> Vec<&'static str>;
    fn get_name(&self) -> &'static str;
    fn run_extractor(&self, temp_dir: &Path, dir_file: &str) -> Result<std::process::Output>;
    fn get_expected_files(&self) -> HashSet<String>;
}

pub struct JonssonMjolner;
pub struct JonssonDjupet;
pub struct MulleBil;

impl GameExtractor for JonssonMjolner {
    fn get_name(&self) -> &'static str {
        "Jönssonligan: Jakten på Mjölner"
    }

    fn prepare_temp_directory(&self, temp_dir: &Path) -> Result<()> {
        prepare_jonsson_temp_directory(temp_dir)
    }

    fn run_extractor(&self, temp_dir: &Path, dir_file: &str) -> Result<std::process::Output> {
        run_extractor_common(temp_dir, dir_file)
    }

    fn get_transparent_color(&self) -> [u8; 3] {
        [255, 255, 255] // White
    }

    fn post_extraction_setup(
        &self,
        temp_dir: &Path,
        processed_files: &[(PathBuf, ImageFormat)],
    ) -> Result<()> {
        for (temp_path, format) in processed_files {
            let extension = format.extensions_str()[0];
            if temp_path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with("berlin--Animationer__vanheden700")
            {
                let new_file_name = format!("berlin--Animationer__vanheden707.{}", extension);
                let new_path = temp_dir.join(new_file_name);

                if temp_path.exists() {
                    fs::copy(&temp_path, &new_path)
                        .context(format!("Failed to copy file: {:?}", temp_path))?;
                }
            }
        }
        Ok(())
    }

    fn get_broken_images(&self) -> Vec<&'static str> {
        vec![
            "berlin--Animationer__harry0000-166.bmp",
            "berlin--Animationer__ingo0000-80.bmp",
            "berlin--Animationer__ingo0041-121.bmp",
            "berlin--Animationer__ingo0042-122.bmp",
            "berlin--Animationer__sickan0000-37.bmp",
            "berlin--Animationer__sickan0001-38.bmp",
            "berlin--Animationer__sickan0042.bmp",
            "berlin--Animationer__vanheden0000-123.bmp",
            "berlin--Animationer__vanheden0042-165.bmp",
        ]
    }

    fn get_expected_files(&self) -> HashSet<String> {
        [
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
        .collect()
    }
}

impl GameExtractor for JonssonDjupet {
    fn get_name(&self) -> &'static str {
        "Jönssonligan: Går på djupet"
    }

    fn prepare_temp_directory(&self, temp_dir: &Path) -> Result<()> {
        prepare_jonsson_temp_directory(temp_dir)?;

        // Handle Xtras folder
        let xtras_src = temp_dir.join("xtras");
        let xtras_dst = temp_dir.join("Xtras");
        copy_directory(&xtras_src, &xtras_dst)
            .context("Failed to copy xtras folder contents to Xtras")?;
        fs::remove_dir_all(&xtras_src).context("Failed to remove xtras folder")?;
        Ok(())
    }

    fn run_extractor(&self, temp_dir: &Path, dir_file: &str) -> Result<std::process::Output> {
        run_extractor_common(temp_dir, dir_file)
    }

    fn get_transparent_color(&self) -> [u8; 3] {
        [255, 0, 255] // Purple
    }

    fn post_extraction_setup(
        &self,
        _temp_dir: &Path,
        _processed_files: &[(PathBuf, ImageFormat)],
    ) -> Result<()> {
        // No additional setup needed for Jönssonligan: Går på djupet
        Ok(())
    }

    fn get_broken_images(&self) -> Vec<&'static str> {
        vec!["Mainmenu--Internal__m_birdanim2_12-473.bmp"]
    }

    fn get_expected_files(&self) -> HashSet<String> {
        ["avi.dir", "game.dir", "mainmenu.dir", "qt.dir"]
            .iter()
            .map(|&s| s.to_lowercase())
            .collect()
    }
}

impl GameExtractor for MulleBil {
    fn get_name(&self) -> &'static str {
        "Mulle Meck bygger bilar"
    }

    fn prepare_temp_directory(&self, temp_dir: &Path) -> Result<()> {
        let xtras_src = temp_dir.join("xtras");
        let xtras_dst = temp_dir.join("Xtras");

        copy_directory(&xtras_src, &xtras_dst)
            .context("Failed to copy xtras folder contents to Xtras")?;
        fs::remove_dir_all(&xtras_src).context("Failed to remove xtras folder")?;

        let movies_src = temp_dir.join("movies");

        copy_directory(&movies_src, &temp_dir)
            .context("Failed to copy movies folder contents to temp dir")?;

        let data_src = temp_dir.join("data");
        copy_directory(&data_src, &temp_dir)
            .context("Failed to copy data folder contents to temp dir")?;

        Ok(())
    }

    fn get_transparent_color(&self) -> [u8; 3] {
        [0, 0, 0]
    }

    fn post_extraction_setup(
        &self,
        _temp_dir: &Path,
        _processed_files: &[(PathBuf, ImageFormat)],
    ) -> Result<()> {
        Ok(())
    }

    fn get_broken_images(&self) -> Vec<&'static str> {
        vec!["02--00__Dummy-2.bmp"]
    }

    fn run_extractor(&self, temp_dir: &Path, dir_file: &str) -> Result<std::process::Output> {
        #[cfg(target_os = "windows")]
        {
            run_extractor_common(temp_dir, dir_file)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let temp_dir = temp_dir.to_path_buf();
            let dir_file = dir_file.to_string();

            let running = Arc::new(AtomicBool::new(true));
            let running_clone = running.clone();

            let extractor_thread = thread::spawn(move || {
                Command::new("wine")
                    .arg("dir_extractor.exe")
                    .arg(&dir_file)
                    .current_dir(&temp_dir)
                    .output()
            });

            // When we run the dir_extractor Mulle Meck bygger bilar will
            // throw a Director Player Error dialog that needs to be dismissed
            // but the extract process will still work as long as we just dismiss
            // the error dialogs. We can use xdotool to press Enter to dismiss the dialog.
            let xdotool_thread = thread::spawn(move || {
                while running_clone.load(Ordering::SeqCst) {
                    let output = Command::new("xdotool").args(&["key", "Return"]).output();

                    match output {
                        Ok(o) => {
                            if !o.stdout.is_empty() {
                                println!("xdotool output: {}", String::from_utf8_lossy(&o.stdout));
                            }
                            if !o.stderr.is_empty() {
                                eprintln!("xdotool error: {}", String::from_utf8_lossy(&o.stderr));
                            }
                        }
                        Err(e) => eprintln!("Failed to run xdotool: {}", e),
                    }

                    thread::sleep(Duration::from_millis(250));
                }
            });

            let result = extractor_thread.join().unwrap()?;

            // Signal the xdotool thread to stop
            running.store(false, Ordering::SeqCst);

            // Wait a bit for the xdotool thread to finish its last iteration
            thread::sleep(Duration::from_millis(500));

            // Now we can safely join the xdotool thread
            if let Err(e) = xdotool_thread.join() {
                eprintln!("Error joining xdotool thread: {:?}", e);
            }

            Ok(result)
        }
    }

    fn get_expected_files(&self) -> HashSet<String> {
        [
            "02.dxr",
            "03.dxr",
            "04.dxr",
            "05.dxr",
            "06.dxr",
            "08.dxr",
            "10.dxr",
            "12.dxr",
            "13.dxr",
            "18.dxr",
            "82.dxr",
            "83.dxr",
            "84.dxr",
            "85.dxr",
            "86.dxr",
            "87.dxr",
            "88.dxr",
            "89.dxr",
            "90.dxr",
            "91.dxr",
            "92.dxr",
            "93.dxr",
            "94.dxr",
            "lbstart.dxr",
            "unload.dxr",
        ]
        .iter()
        .map(|&s| s.to_lowercase())
        .collect()
    }
}

fn run_extractor_common(temp_dir: &Path, dir_file: &str) -> Result<std::process::Output> {
    #[cfg(target_os = "windows")]
    {
        Command::new(temp_dir.join("dir_extractor.exe"))
            .arg(dir_file)
            .current_dir(temp_dir)
            .output()
            .context("Failed to run extractor on Windows")
    }
    #[cfg(not(target_os = "windows"))]
    {
        Command::new("wine")
            .arg("dir_extractor.exe")
            .arg(dir_file)
            .current_dir(temp_dir)
            .output()
            .context("Failed to run extractor with Wine")
    }
}

pub fn copy_directory(src: &Path, dst: &Path) -> Result<()> {
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

fn prepare_jonsson_temp_directory(temp_dir: &Path) -> Result<()> {
    // Handle case-insensitive data directory
    let data_dir = fs::read_dir(temp_dir)?
        .filter_map(Result::ok)
        .find(|entry| {
            let path = entry.path();
            path.is_dir()
                && path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_lowercase() == "data")
                    .unwrap_or(false)
        })
        .map(|entry| entry.path());

    if let Some(data_dir) = data_dir {
        for entry in fs::read_dir(&data_dir)? {
            let entry = entry?;
            let src_path = entry.path();
            let dst_path = temp_dir.join(entry.file_name());

            if src_path.is_dir() {
                copy_directory(&src_path, &dst_path)
                    .context(format!("Failed to copy directory: {:?}", src_path))?;
            } else {
                fs::copy(&src_path, &dst_path)
                    .context(format!("Failed to copy file: {:?}", src_path))?;
            }
        }
    }

    Ok(())
}
