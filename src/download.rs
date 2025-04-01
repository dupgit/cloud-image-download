use crate::checksums;
use crate::website::WSImageList;
use crate::{CID_USER_AGENT, CONCURRENT_REQUESTS, checksums::CheckSums};
use clap_verbosity_flag::Verbosity;
use colored::Colorize;
use hex_literal::hex;
use log::{error, info, warn};
use reqwest::Url;
use reqwest::header::{HeaderValue, USER_AGENT};
use sha2::{Digest, Sha256, Sha512};
use std::error::{self, Error};
use std::fs::File;
use std::fs::create_dir_all;
use std::io::{BufReader, Read};
use std::path::PathBuf;
use trauma::download::Status;
use trauma::{
    download::{Download, Summary},
    downloader::{Downloader, DownloaderBuilder},
};

// Creates a directory if it does not exists. Returns Ok unless create_dir_all() fails.
fn create_dir_if_needed(path: &PathBuf) -> Result<(), Box<dyn Error>> {
    if !path.exists() {
        create_dir_all(path)?;
    }
    Ok(())
}

fn get_filename_destination(image_name: &str, file_destination: &PathBuf) -> Option<(Url, String)> {
    match Url::parse(image_name) {
        Ok(url) => {
            if let Some(image_name) = image_name.split('/').last() {
                match create_dir_if_needed(file_destination) {
                    Ok(_) => {
                        if let Some(filename) = file_destination.join(image_name).to_str() {
                            return Some((url, filename.to_string()));
                        } else {
                            warn!("{image_name} is not a valid UTF-8 string");
                        }
                    }
                    Err(e) => {
                        warn!("Error '{e}' while creating destination {:?} directory", file_destination);
                    }
                }
            } else {
                warn!("Failed to get filename from {} - skipped", image_name);
            }
        }
        Err(e) => warn!("Error {e}: skipped url {}", image_name),
    }

    None
}

pub async fn download_images(
    all_ws_image_lists: &Vec<WSImageList>,
    verbose: &Verbosity,
    concurrent_download: usize,
) -> Vec<Summary> {
    let mut download_image_list = vec![];

    // Building the download list with destination directory
    for ws_image in all_ws_image_lists {
        for cloud_image in &ws_image.images_list.list {
            if let Some((url, filename)) = get_filename_destination(&cloud_image.name, &ws_image.website.destination) {
                info!("Will try to download {url} to {filename}");
                download_image_list.push(Download::new(&url, &filename));
            }
        }
    }

    let downloader: Downloader;
    let retry = 3;
    let user_agent = HeaderValue::from_str(CID_USER_AGENT).unwrap();

    // Defines the maxuimum simultaneous downloads at a time
    let max: usize;
    if concurrent_download > 0 {
        max = concurrent_download;
    } else {
        max = CONCURRENT_REQUESTS;
    }

    // Prepares downloading
    if verbose.is_silent() {
        // Without any progress bars (needs -q verbosity option)
        downloader =
            DownloaderBuilder::hidden().concurrent_downloads(max).header(USER_AGENT, user_agent).retries(retry).build();
    } else {
        // With progress bars (no -q or one or more -v verbosity options)
        downloader =
            DownloaderBuilder::new().concurrent_downloads(max).header(USER_AGENT, user_agent).retries(retry).build();
    }

    // Downloads effectively
    downloader.download(&download_image_list).await
}

/// This will display a summary only on info log level
pub fn display_download_status_summary(downloaded_summary: Vec<Summary>) {
    // Prepares file list to be checked
    for summary in downloaded_summary {
        let download = summary.download();
        match summary.status() {
            Status::Success => {
                info!("{} Successfully downloaded {}", "🗸".green(), download.filename);
            }
            Status::Fail(e) => {
                info!("{} Error '{e}' while downloading {} to {}", "𐄂".red(), download.url, download.filename)
            }
            Status::Skipped(e) => {
                // Probably already downloaded
                info!("{} Skipped {} to be downloaded from {}: '{e}' ", "🗸".green(), download.filename, download.url)
            }
            Status::NotStarted => {
                info!("{} Downloading {} to {} has not been started", "𐄂".red(), download.url, download.filename)
            }
        }
    }
}

pub fn verify_file(filename: &str, checksum: &CheckSums) -> Option<bool> {
    let input = match File::open(filename) {
        Ok(input) => input,
        Err(e) => {
            error!("Error while opening {filename}: {e}");
            return None;
        }
    };

    let mut reader = BufReader::new(input);

    match checksum {
        CheckSums::None => {
            warn!("No checksum for file {filename}: nothing verified");
            return None;
        }
        CheckSums::Sha256(hash) => {
            info!("Verifying {filename} sha256's checksum");
            let digest = {
                let mut hasher = Sha256::new();
                let mut buffer = vec![0; 16_777_216];
                loop {
                    if let Ok(count) = reader.read(&mut buffer) {
                        if count == 0 {
                            break;
                        }
                        hasher.update(&buffer[..count]);
                    } else {
                        error!("Error while reading file {filename} Skipped");
                        return None;
                    }
                }
                hasher.finalize()
            };
            if base16ct::lower::encode_string(&digest) == *hash {
                return Some(true);
            } else {
                return Some(false);
            }
        }
        CheckSums::Sha512(hash) => {
            info!("Verifying {filename} sha512's checksum");
            let digest = {
                let mut hasher = Sha512::new();
                let mut buffer = vec![0; 16_777_216];
                loop {
                    if let Ok(count) = reader.read(&mut buffer) {
                        if count == 0 {
                            break;
                        }
                        hasher.update(&buffer[..count]);
                    } else {
                        error!("Error while reading file {filename} Skipped");
                        return None;
                    }
                }
                hasher.finalize()
            };
            if base16ct::lower::encode_string(&digest) == *hash {
                return Some(true);
            } else {
                return Some(false);
            }
        }
    }
}

pub fn verify_downloaded_file(all_ws_image_lists: &Vec<WSImageList>) {
    for ws_image in all_ws_image_lists {
        for cloud_image in &ws_image.images_list.list {
            if let Some((_, filename)) = get_filename_destination(&cloud_image.name, &ws_image.website.destination) {
                match verify_file(&filename, &cloud_image.checksum) {
                    Some(success) => {
                        if success {
                            info!("{} Successfully verified {filename}", "🗸".green());
                        } else {
                            warn!("{} Verifying failed for {filename}", "𐄂".red())
                        }
                    }
                    None => warn!("{} {filename} not verified.", "𐄂".red()),
                }
            }
        }
    }
}
