use crate::image_history::DbImageHistory;
use crate::website::WSImageList;
use crate::{CID_USER_AGENT, CONCURRENT_REQUESTS};
use clap_verbosity_flag::Verbosity;
use colored::Colorize;
use log::{error, info, warn};
use reqwest::Url;
use reqwest::header::{HeaderValue, USER_AGENT};
use std::error::Error;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::task;
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

pub fn get_filename_destination(image_url: &str, file_destination: &PathBuf) -> Option<(Url, String)> {
    match Url::parse(image_url) {
        Ok(url) => {
            if let Some(image_name) = image_url.split('/').last() {
                if let Some(filename) = file_destination.join(image_name).to_str() {
                    return Some((url, filename.to_string()));
                } else {
                    warn!("{image_name} is not a valid UTF-8 string");
                }
            } else {
                warn!("Failed to get filename from {} - skipped", image_url);
            }
        }
        Err(e) => warn!("Error {e}: skipped url {}", image_url),
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
            match create_dir_if_needed(&ws_image.website.destination) {
                Ok(_) => {
                    if let Some((url, filename)) =
                        get_filename_destination(&cloud_image.url, &ws_image.website.destination)
                    {
                        info!("Will try to download {url} to {filename}");
                        download_image_list.push(Download::new(&url, &filename));
                    }
                }
                Err(e) => {
                    warn!("Error '{e}' while creating destination {:?} directory", &ws_image.website.destination);
                }
            }
        }
    }

    let downloader: Downloader;
    let retry = 3;
    let user_agent = HeaderValue::from_str(CID_USER_AGENT).unwrap();

    // Defines the maximum simultaneous downloads at a time
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

/// This will display a summary of what went correctly
/// and what did not only if -q was not selected
pub fn display_download_status_summary(downloaded_summary: &Vec<Summary>, verbose: &Verbosity) {
    if !verbose.is_silent() {
        // If -q hasn't been selected
        // Prepares file list to be checked
        for summary in downloaded_summary {
            let download = summary.download();
            match summary.status() {
                Status::Success => {
                    println!("{} Successfully downloaded {}", "🗸".green(), download.filename);
                }
                Status::Fail(e) => {
                    println!("{} Error '{e}' while downloading {} to {}", "𐄂".red(), download.url, download.filename)
                }
                Status::Skipped(e) => {
                    // Probably already downloaded
                    println!(
                        "{} Skipped {} to be downloaded from {}: '{e}' ",
                        "🗸".green(),
                        download.filename,
                        download.url
                    )
                }
                Status::NotStarted => {
                    println!("{} Downloading {} to {} has not been started", "𐄂".red(), download.url, download.filename)
                }
            }
        }
    }
}

/// This will tell if an image has effectively been downloaded
pub fn image_has_been_downloaded(downloaded_summary: &Vec<Summary>, image_url: &str, destination: &PathBuf) -> bool {
    for summary in downloaded_summary {
        let download = summary.download();
        if let Some((url, filename)) = get_filename_destination(image_url, destination) {
            if download.filename == filename && download.url == url {
                match summary.status() {
                    Status::Success => {
                        info!("Keeping image {filename} from {url}");
                        return true;
                    }
                    Status::Fail(_) | Status::Skipped(_) | Status::NotStarted => {
                        return false;
                    }
                }
            }
        }
    }
    false
}

/// @todo: Does not limits itself to the numbers of tasks corresponding to
/// concurrent_downloads command line option
pub async fn verify_downloaded_file(all_ws_image_lists: Vec<WSImageList>, db: Arc<DbImageHistory>) {
    let mut join_handle_list = Vec::new();

    for ws_image in all_ws_image_lists {
        for cloud_image in ws_image.images_list.list {
            let website = ws_image.website.clone();
            let join_handle = task::spawn(async move {
                if cloud_image.verify(&website.destination) {
                    Some(cloud_image)
                } else {
                    None
                }
            });
            join_handle_list.push(join_handle);
        }
    }

    for join_handle in join_handle_list {
        match join_handle.await {
            Ok(option_cloud_image) => match option_cloud_image {
                Some(cloud_image) => db.save_image_in_db(&cloud_image),
                None => (),
            },
            Err(e) => error!("Error in task: {e}"),
        }
    }
}
