use crate::cloud_image::CloudImage;
use crate::image_history::DbImageHistory;
use crate::website::WSImageList;
use crate::{CID_USER_AGENT, CONCURRENT_REQUESTS};
use clap_verbosity_flag::Verbosity;
use colored::Colorize;
use http::header::HeaderMap;
use log::{error, info, warn};
use reqwest::Url as ReqwestUrl;
use reqwest::header::{HeaderValue, USER_AGENT};
use std::error::Error;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::task;
use trauma::{
    download::{Download, Status, Summary},
    downloader::{Downloader, StyleOptions},
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

// @todo: rewrite this doc
/// With `image_name` as a filename and `file_destination` as a Path
/// returns the absolute file name. If `normalized` is true then
/// uses `image_date` to format the name of the file with the date
/// inserted right before the dot (ie for `example.qcow2` you will
/// get `example-20250710.qcow2` when normalized)
#[must_use]
pub fn get_filename_destination(
    cloud_image: &CloudImage,
    file_destination: &Path,
    normalize: &Option<String>,
) -> Option<String> {
    let image_date = cloud_image.date;
    let image_name = &cloud_image.name;
    let version = match &cloud_image.url.version {
        Some(v) => v,
        None => "",
    };
    let after_version = match &cloud_image.url.after_version {
        Some(av) => av,
        None => "",
    };

    let normalized_image_name: String;

    if normalize.is_some()
        && let Some(normalize_string) = normalize
    {
        normalized_image_name = normalize_string
            .replace("{date}", &image_date.format("%Y%m%d").to_string())
            .replace("{version}", version)
            .replace("{after_version}", after_version);
    } else {
        normalized_image_name = image_name.clone();
    }

    if let Some(filename) = file_destination.join(&normalized_image_name).to_str() {
        Some(filename.to_string())
    } else {
        warn!("{normalized_image_name} is not a valid UTF-8 string");
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
                    if let Some(filename) = get_filename_destination(
                        cloud_image,
                        &ws_image.website.destination,
                        &ws_image.website.normalize,
                    ) {
                        match ReqwestUrl::parse(&cloud_image.url.url) {
                            Ok(url) => {
                                info!("Will try to download '{}' to {filename}", cloud_image.url.url);
                                download_image_list.push(
                                    Download::builder()
                                        .url(url)
                                        .expect("Url '{url}' is known to be good")
                                        .filename_override(&filename)
                                        .build(),
                                );
                            }
                            Err(e) => {
                                error!("Can not transform '{}' into reqwest Url type: {e}", cloud_image.url.url);
                                info!("As a result {filename} will not be downloaded");
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!(
                        "Error '{e}' while creating destination {} directory",
                        ws_image.website.destination.display()
                    );
                }
            }
        }
    }

    let retry = 3;
    let user_agent =
        HeaderValue::from_str(CID_USER_AGENT).expect("Converting CID_USER_AGENT to HeaderValue should not fail");

    // Defines the maximum simultaneous downloads at a time
    let max: usize = if concurrent_download > 0 {
        concurrent_download
    } else {
        CONCURRENT_REQUESTS
    };

    let mut headers = HeaderMap::new();
    headers.insert(USER_AGENT, user_agent);

    // Prepares downloading
    let downloader: Downloader = if verbose.is_silent() {
        // Without any progress bars (needs -q verbosity option)
        let style_options = StyleOptions::hidden();
        Downloader::builder()
            .concurrent_downloads(max)
            .headers(headers)
            .retries(retry)
            .style_options(style_options)
            .build()
    } else {
        // With progress bars (no -q or one or more -v verbosity options)
        Downloader::builder().concurrent_downloads(max).headers(headers).retries(retry).build()
    };

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
            let filename = match download.filename_override() {
                Some(filename) => filename,
                None => &String::new(),
            };
            match summary.status() {
                Status::Success => {
                    println!("{} Successfully downloaded {}", "🗸".green(), filename);
                }
                Status::Fail(e) => {
                    println!("{} Error '{e}' while downloading {} to {}", "𐄂".red(), download.url(), filename);
                }
                Status::Skipped(e) => {
                    // Probably already downloaded
                    println!("{} Skipped {} to be downloaded from {}: '{e}' ", "🗸".green(), filename, download.url());
                }
                Status::NotStarted => {
                    println!("{} Downloading {} to {} has not been started", "𐄂".red(), download.url(), filename);
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
    downloaded_summary: &[Summary],
    cloud_image: &CloudImage,
    destination: &Path,
    verify_skipped: bool,
    normalize: &Option<String>,
) -> bool {
    let Some(filename) = get_filename_destination(cloud_image, destination, normalize) else {
        return false;
    };

    let url = match ReqwestUrl::parse(&cloud_image.url.url) {
        Ok(url) => url,
        Err(e) => {
            error!("Can not transform '{}' into reqwest Url type: {e}", cloud_image.url.url);
            return false;
        }
    };

    for summary in downloaded_summary {
        let download = summary.download();

        let Some(download_filename) = download.filename_override() else {
            warn!(
                "Download for '{filename}' (image '{}', url '{}') has no filename_override — \
                 this should never happen since every Download in this crate is built \
                 with one; skipping this summary entry",
                cloud_image.name, cloud_image.url.url
            );
            continue;
        };

        if download_filename == &filename && download.url() == url {
            return match summary.status() {
                Status::Success => {
                    info!("Keeping image {filename} from {url}");
                    true
                }
                Status::Fail(_) | Status::NotStarted => false,
                Status::Skipped(_) => verify_skipped,
            };
        }
    }
    false
}

/// @todo: Does not limits itself to the numbers of tasks corresponding to
/// `concurrent_downloads` command line option
pub async fn verify_downloaded_file(
    all_ws_image_lists: Vec<WSImageList>,
    db: Arc<DbImageHistory>,
    downloaded_summary: &[Summary],
    verify_skipped: bool,
) {
    let mut join_handle_list = Vec::new();

    for ws_image in all_ws_image_lists {
        for cloud_image in ws_image.images_list {
            // Checks that the image has been downloaded effectively
            // before checking its checksum
            if image_has_been_downloaded(
                downloaded_summary,
                &cloud_image,
                &ws_image.website.destination,
                verify_skipped,
                &ws_image.website.normalize,
            ) {
                let website = ws_image.website.clone();
                let join_handle = task::spawn(async move {
                    if cloud_image.verify(&website.destination, &website.normalize) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::checksums::CheckSums;
    use crate::website::Url;
    use chrono::NaiveDateTime;
    use trauma::download::{Download, Status, Summary};

    fn dummy_real_url() -> Url {
        Url::new(
            "http://cloud.centos.org/centos".to_string(),
            Some("10-stream".to_string()),
            Some("x86_64/images".to_string()),
        )
    }

    fn dummy_cloud_image() -> CloudImage {
        CloudImage::new(
            dummy_real_url(),
            CheckSums::Sha256("abc123".into()),
            "foo.qcow2".to_string(),
            NaiveDateTime::parse_from_str("2026-08-30 10:00:00", "%Y-%m-%d %H:%M:%S").unwrap(),
        )
    }

    fn make_download_with_override(url: &str, filename_override: &str) -> Download {
        Download::builder().url(url).unwrap().filename_override(filename_override).build()
    }

    fn make_download_without_override(url: &str) -> Download {
        Download::builder().url(url).unwrap().build()
    }

    #[test]
    fn test_empty_summary_list_returns_false() {
        let cloud_image = dummy_cloud_image();
        let destination = Path::new("/tmp/downloads");
        assert!(!image_has_been_downloaded(&vec![], &cloud_image, destination, false, &None));
    }

    #[test]
    fn test_matching_summary_with_success_returns_true() {
        let cloud_image = dummy_cloud_image();
        let destination = Path::new("/tmp/downloads");
        let filename = get_filename_destination(&cloud_image, destination, &None).unwrap();

        let download = make_download_with_override(&cloud_image.url.url, &filename);
        let summary = Summary::builder().download(download).status(Status::Success).build();

        assert!(image_has_been_downloaded(&vec![summary], &cloud_image, destination, false, &None));
    }

    #[test]
    fn test_matching_summary_with_fail_returns_false() {
        let cloud_image = dummy_cloud_image();
        let destination = Path::new("/tmp/downloads");
        let filename = get_filename_destination(&cloud_image, destination, &None).unwrap();

        let download = make_download_with_override(&cloud_image.url.url, &filename);
        let summary = Summary::builder().download(download).status(Status::Fail("network error".to_string())).build();

        assert!(!image_has_been_downloaded(&vec![summary], &cloud_image, destination, false, &None));
    }

    #[test]
    fn test_matching_summary_with_not_started_returns_false() {
        let cloud_image = dummy_cloud_image();
        let destination = Path::new("/tmp/downloads");
        let filename = get_filename_destination(&cloud_image, destination, &None).unwrap();

        let download = make_download_with_override(&cloud_image.url.url, &filename);
        let summary = Summary::builder().download(download).status(Status::NotStarted).build();

        assert!(!image_has_been_downloaded(&vec![summary], &cloud_image, destination, false, &None));
    }

    #[test]
    fn test_matching_summary_skipped_follows_verify_skipped_flag() {
        let cloud_image = dummy_cloud_image();
        let destination = Path::new("/tmp/downloads");
        let filename = get_filename_destination(&cloud_image, destination, &None).unwrap();

        let download_true = make_download_with_override(&cloud_image.url.url, &filename);
        let summary_true =
            Summary::builder().download(download_true).status(Status::Skipped("already exists".to_string())).build();
        assert!(image_has_been_downloaded(&vec![summary_true], &cloud_image, destination, true, &None));

        let download_false = make_download_with_override(&cloud_image.url.url, &filename);
        let summary_false =
            Summary::builder().download(download_false).status(Status::Skipped("already exists".to_string())).build();
        assert!(!image_has_been_downloaded(&vec![summary_false], &cloud_image, destination, false, &None));
    }

    #[test]
    fn test_missing_filename_override_is_skipped_defensively() {
        let cloud_image = dummy_cloud_image();
        let destination = Path::new("/tmp/downloads");

        let download = make_download_without_override(&cloud_image.url.url);
        let summary = Summary::builder().download(download).status(Status::Success).build();

        assert!(!image_has_been_downloaded(&vec![summary], &cloud_image, destination, false, &None));
    }

    #[test]
    fn test_invalid_cloud_image_url_returns_false() {
        let mut cloud_image = dummy_cloud_image();
        cloud_image.url.url = "not a valid url".to_string();
        let destination = Path::new("/tmp/downloads");

        let download = make_download_without_override("https://example.com/foo.qcow2");
        let summary = Summary::builder().download(download).status(Status::Success).build();

        assert!(!image_has_been_downloaded(&vec![summary], &cloud_image, destination, false, &None));
    }

    #[test]
    fn test_iterates_past_non_matching_summaries() {
        let cloud_image = dummy_cloud_image();
        let destination = Path::new("/tmp/downloads");
        let filename = get_filename_destination(&cloud_image, destination, &None).unwrap();

        let non_matching = make_download_with_override(&cloud_image.url.url, "wrong-name.qcow2");
        let non_matching_summary = Summary::builder().download(non_matching).status(Status::Success).build();

        let matching = make_download_with_override(&cloud_image.url.url, &filename);
        let matching_summary = Summary::builder().download(matching).status(Status::Success).build();

        assert!(image_has_been_downloaded(
            &vec![non_matching_summary, matching_summary],
            &cloud_image,
            destination,
            false,
            &None
        ));
    }
}
