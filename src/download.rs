use crate::website::WSImageList;
use crate::{CID_USER_AGENT, CONCURRENT_REQUESTS, checksums::CheckSums};
use clap_verbosity_flag::Verbosity;
use log::{info, warn};
use reqwest::Url;
use reqwest::header::{HeaderValue, USER_AGENT};
use std::error::Error;
use std::fs::create_dir_all;
use std::path::PathBuf;
use trauma::{download::Download, downloader::Downloader, downloader::DownloaderBuilder};

// Creates a directory if it does not exists. Returns Ok unless create_dir_all() fails.
fn create_dir_if_needed(path: &PathBuf) -> Result<(), Box<dyn Error>> {
    if !path.exists() {
        create_dir_all(path)?;
    }
    Ok(())
}

pub async fn download_images(all_ws_image_lists: Vec<WSImageList>, verbose: &Verbosity, concurrent_download: usize) {
    let mut download_image_list = vec![];

    // Building the download list with destination directory
    for ws_image in all_ws_image_lists {
        for cloud_image in ws_image.images_list.list {
            match Url::parse(&cloud_image.name) {
                Ok(url) => {
                    if let Some(image_name) = cloud_image.name.split('/').last() {
                        match create_dir_if_needed(&ws_image.website.destination) {
                            Ok(_) => {
                                if let Some(filename) = ws_image.website.destination.join(image_name).to_str() {
                                    info!("Will try to download {url} to {filename}");
                                    download_image_list.push(Download::new(&url, filename));
                                } else {
                                    warn!("{image_name} is not a valid UTF-8 string");
                                }
                            }
                            Err(e) => {
                                warn!(
                                    "Error '{e}' while creating destination {:?} directory",
                                    ws_image.website.destination
                                );
                            }
                        }
                    } else {
                        warn!("Failed to get filename from {} - skipped", cloud_image.name);
                    }
                }
                Err(e) => warn!("Error {e}: skipped url {}", cloud_image.name),
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
    downloader.download(&download_image_list).await;
}
