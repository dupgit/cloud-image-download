use crate::cloud_image::CloudImage;
use crate::image_history::DbImageHistory;
use crate::website::WSImageList;
use crate::{CID_USER_AGENT, CONCURRENT_REQUESTS};
use chrono::NaiveDateTime;
use clap_verbosity_flag::Verbosity;
use colored::Colorize;
use log::{error, info, warn};
use reqwest::Url;
use reqwest::header::{HeaderValue, USER_AGENT};
use std::error::Error;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task;
use trauma::download::Status;
use trauma::{
    download::{Download, Summary},
    downloader::{Downloader, DownloaderBuilder},
};

/// Creates a directory if it does not exists. Returns Ok unless `create_dir_all()` fails.
///
/// # Errors
///
/// Returned errors are the ones that `std::fs::create_dir_all()` method
/// returns
fn create_dir_if_needed(path: &PathBuf) -> Result<(), Box<dyn Error>> {
    if !path.exists() {
        create_dir_all(path)?;
    }
    Ok(())
}

/// With `image_name` as a filename and `file_destination` as a Path
/// returns the absolute file name. If `normalized` is true then
/// uses `image_date` to format the name of the file with the date
/// inserted right before the dot (ie for `example.qcow2` you will
/// get `example-20250710.qcow2` when normalized)
#[must_use]
pub fn get_filename_destination(
    image_name: &str,
    file_destination: &Path,
    normalize: bool,
    image_date: NaiveDateTime,
) -> Option<String> {
    let mut normalized_image_name: String = image_name.to_string();

    if normalize && let Some((first_part, last_part)) = image_name.rsplit_once('.') {
        normalized_image_name = format!("{first_part}-{}.{last_part}", image_date.format("%Y%m%d"));
    }
    if let Some(filename) = file_destination.join(normalized_image_name).to_str() {
        Some(filename.to_string())
    } else {
        warn!("{image_name} is not a valid UTF-8 string");
        None
    }
}

/// Download images:
/// - first creates all destination directories if needed and
///   adds every file to a `download_image_list` with `Download`
///   entries for trauma
/// - Builds the downloader with some options such as the number
///   of concurrent downloads, if we may display progress bar or
///   not
/// - At last downloads effectively the image files
///
/// # Panics
///
/// Only if the converted `CARGO_PKG_VERSION` can not be converted
/// into a `HeaderValue` (`CID_USER_AGENT`) and this should never
/// happen.
pub async fn download_images(
    all_ws_image_lists: &Vec<WSImageList>,
    verbose: &Verbosity,
    concurrent_download: usize,
) -> Vec<Summary> {
    let mut download_image_list = vec![];

    // Building the download list with destination directory
    for ws_image in all_ws_image_lists {
        for cloud_image in &ws_image.images_list {
            match create_dir_if_needed(&ws_image.website.destination) {
                Ok(()) => {
                    let normalize = ws_image.website.get_normalize();
                    if let Some(filename) = get_filename_destination(
                        &cloud_image.name,
                        &ws_image.website.destination,
                        normalize,
                        cloud_image.date,
                    ) {
                        match Url::parse(&cloud_image.url) {
                            Ok(url) => {
                                info!("Will try to download '{}' to {filename}", cloud_image.url);
                                download_image_list.push(Download::new(&url, &filename));
                            }
                            Err(e) => {
                                error!("Can not transform '{}' into reqwest Url type: {e}", cloud_image.url);
                                info!("As a result {filename} will not be downloaded");
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "Error '{e}' while creating destination {} directory",
                        &ws_image.website.destination.display()
                    );
                }
            }
        }
    }

    let downloader: Downloader;
    let retry = 3;
    let user_agent =
        HeaderValue::from_str(CID_USER_AGENT).expect("Converting CID_USER_AGENT to HeaderValue should not fail");

    // Defines the maximum simultaneous downloads at a time
    let max: usize = if concurrent_download > 0 {
        concurrent_download
    } else {
        CONCURRENT_REQUESTS
    };

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
                    println!("{} Error '{e}' while downloading {} to {}", "𐄂".red(), download.url, download.filename);
                }
                Status::Skipped(e) => {
                    // Probably already downloaded
                    println!(
                        "{} Skipped {} to be downloaded from {}: '{e}' ",
                        "🗸".green(),
                        download.filename,
                        download.url
                    );
                }
                Status::NotStarted => {
                    println!(
                        "{} Downloading {} to {} has not been started",
                        "𐄂".red(),
                        download.url,
                        download.filename
                    );
                }
            }
        }
    }
}

/// This will tell if an image has effectively been downloaded
/// using the summary that trauma gives at the end of the
/// process
#[must_use]
pub fn image_has_been_downloaded(
    downloaded_summary: &Vec<Summary>,
    cloud_image: &CloudImage,
    destination: &Path,
    verify_skipped: bool,
    normalize: bool,
) -> bool {
    for summary in downloaded_summary {
        let download = summary.download();
        if let Some(filename) = get_filename_destination(&cloud_image.name, destination, normalize, cloud_image.date) {
            match Url::parse(&cloud_image.url) {
                Ok(url) => {
                    if download.filename == filename && download.url == url {
                        match summary.status() {
                            Status::Success => {
                                info!("Keeping image {filename} from {url}");
                                return true;
                            }
                            Status::Fail(_) | Status::NotStarted => {
                                return false;
                            }
                            Status::Skipped(_) => {
                                return verify_skipped;
                            }
                        }
                    }
                }
                Err(e) => {
                    error!("Can not transform '{}' into reqwest Url type: {e}", cloud_image.url);
                }
            }
        }
    }
    false
}

/// @todo: Does not limits itself to the numbers of tasks corresponding to
/// `concurrent_downloads` command line option
pub async fn verify_downloaded_file(
    all_ws_image_lists: Vec<WSImageList>,
    db: Arc<DbImageHistory>,
    downloaded_summary: &Vec<Summary>,
    verify_skipped: bool,
) {
    let mut join_handle_list = Vec::new();

    for ws_image in all_ws_image_lists {
        for cloud_image in ws_image.images_list {
            // Checks that the image has been downloaded effectively
            // before checking its checksum
            let normalize = ws_image.website.get_normalize();
            if image_has_been_downloaded(
                downloaded_summary,
                &cloud_image,
                &ws_image.website.destination,
                verify_skipped,
                normalize,
            ) {
                let website = ws_image.website.clone();
                let join_handle = task::spawn(async move {
                    if cloud_image.verify(&website.destination, normalize) {
                        Some(cloud_image)
                    } else {
                        None
                    }
                });
                join_handle_list.push(join_handle);
            }
        }
    }

    for join_handle in join_handle_list {
        match join_handle.await {
            Ok(option_cloud_image) => {
                if let Some(cloud_image) = option_cloud_image {
                    db.save_image_in_db(&cloud_image);
                }
            }
            Err(e) => error!("Error in task: {e}"),
        }
    }
}
