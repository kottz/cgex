use anyhow::{Context, Result};
use image::ImageFormat;
use std::fs;
use std::path::{Path, PathBuf};

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
}

pub struct JonssonMjolner;
pub struct JonssonDjupet;

impl GameExtractor for JonssonMjolner {
    fn get_name(&self) -> &'static str {
        "Jönssonligan: Jakten på Mjölner"
    }

    fn prepare_temp_directory(&self, _temp_dir: &Path) -> Result<()> {
        // No additional preparation needed for Jonssonligan: Jakten på Mjölner
        Ok(())
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
}

impl GameExtractor for JonssonDjupet {
    fn get_name(&self) -> &'static str {
        "Jönssonligan: Går på djupet"
    }

    fn prepare_temp_directory(&self, temp_dir: &Path) -> Result<()> {
        let xtras_src = temp_dir.join("xtras");
        let xtras_dst = temp_dir.join("Xtras");

        copy_directory(&xtras_src, &xtras_dst)
            .context("Failed to copy xtras folder contents to Xtras")?;
        fs::remove_dir_all(&xtras_src).context("Failed to remove xtras folder")?;

        Ok(())
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
