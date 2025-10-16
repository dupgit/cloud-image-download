use crate::CID_USER_AGENT;
use crate::checksums::{CheckSums, are_all_checksums_in_one_file};
use crate::cloud_image::CloudImage;
use crate::image_history::DbImageHistory;
use futures::{StreamExt, stream};
use httpdirectory::error::HttpDirError;
use httpdirectory::httpdirectory::{HttpDirectory, Sorting};
use log::{debug, error, info, trace, warn};
use regex::Regex;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;

/// Website description structure
#[derive(Debug, Deserialize)]
pub struct WebSite {
    pub name: String,
    version_list: Vec<String>,
    base_url: String,
    after_version_url: Option<Vec<String>>,
    image_name_filter: String,
    image_name_cleanse: Option<Vec<String>>,
    pub destination: PathBuf,
    pub normalize: Option<String>,
}

/// Associates a list of images with the website
/// they come from
pub struct WSImageList {
    pub images_list: Vec<CloudImage>,
    pub website: Arc<WebSite>,
}

#[derive(Default, PartialEq, Debug)]
pub struct Url {
    pub url: String,
    pub version: Option<String>,
    pub after_version: Option<String>,
}

impl Url {
    #[must_use]
    pub fn new(url: String, version: Option<String>, after_version: Option<String>) -> Self {
        Url {
            url,
            version,
            after_version,
        }
    }
}

/// Retrieves the body of a get request to the specified
/// url and returns Some(body) if everything went fine
/// and None in case of an Error
async fn get_body_from_url(url: &str, client: &reqwest::Client) -> Option<String> {
    match client.get(url).header(ACCEPT, "*/*").header(USER_AGENT, CID_USER_AGENT).send().await {
        Ok(response) => match response.status() {
            reqwest::StatusCode::OK => match response.text().await {
                Ok(body) => Some(body),
                Err(e) => {
                    warn!("Error: no body in response: {e}");
                    None
                }
            },
            _ => {
                warn!("Error while retrieving url '{url}' content ({})", response.status());
                None
            }
        },
        Err(e) => {
            warn!("Error while fetching url '{url}': {e}");
            None
        }
    }
}

fn sanitize_string(unsane: &str) -> String {
    let mut sanitized = unsane.to_string();
    if unsane.ends_with('/') {
        sanitized = sanitized.replace('/', "");
    }
    sanitized.replace('/', "_")
}

impl WebSite {
    /// Generates all url to be checked for images for this particular website
    /// using versions from `version_list` and `after_version_url` that both
    /// are vectors and may contain more than one element.
    /// Checks whether the site has dates directories or numbered directorise
    /// and in that case adds the latest one to the list instead of the url itself.
    /// The returned list may be empty.
    // @todo review this whole process
    async fn generate_url_list(&self) -> Vec<Url> {
        let mut url_list = vec![];
        for version in &self.version_list {
            let url = format!("{}/{}", self.base_url, version);

            if let Some(url_checked) =
                WebSite::check_for_directories_with_dates_or_version_numbers(&self.name, &url, version).await
            {
                if let Some(after) = &self.after_version_url {
                    for after_version in after {
                        // valid url_checked.url has a trailing /
                        let new_url = Url::new(
                            format!("{}{after_version}", url_checked.url),
                            url_checked.version.clone(),
                            Some(sanitize_string(after_version)),
                        );
                        info!("[{}] Adding url '{}' to the list of url", self.name, new_url.url);
                        url_list.push(new_url);
                    }
                } else {
                    info!("[{}] Adding url '{}' to the list of url", self.name, url_checked.url);
                    url_list.push(url_checked);
                }
            } else {
                info!("[{}] Adding url '{url}' to the list of url", self.name);
                url_list.push(Url::new(url, Some(sanitize_string(version)), None));
            }
        }
        url_list
    }

    /// Checks if an url has directories with dates and then returns the url
    /// containing that directory instead of url itself. If the url has no
    /// directories with dates then returns this url
    /// Returns None when `HttpDirectory::new()` returns an Err.
    async fn check_for_directories_with_dates_or_version_numbers(name: &str, url: &str, version: &str) -> Option<Url> {
        if let Ok(directory_listing) = HttpDirectory::new(url).await {
            if let Ok(list_of_dates) = directory_listing.dirs().filter_by_name(r"\d{8}(?:-\d{4})?/$") {
                if list_of_dates.is_empty() {
                    debug!("[{name}] This url ({url}) has no dates in it");
                    if let Ok(list_of_numbers) = directory_listing.dirs().filter_by_name(r"^\d\d+/$") {
                        if list_of_numbers.is_empty() {
                            debug!("[{name}] This url ({url}) has no numbers in it");
                            return Some(Url {
                                url: format!("{url}/"),
                                version: None,
                                after_version: None,
                            });
                        }
                        debug!("[{name}] This url ({url}) has numbers in it:");
                        if let Some((url, number)) = url_with_latest_directory_name(name, list_of_numbers, url) {
                            return Some(Url {
                                url,
                                version: Some(number),
                                after_version: None,
                            });
                        }
                    }
                } else {
                    debug!("[{name}] This url ({url}) has dates in it:");
                    // Keep only the latest entry !
                    if let Some((url, _date)) = url_with_latest_directory_name(name, list_of_dates, url) {
                        // When sanitizing we do not mind to get dates twice
                        // so use the version name instead
                        return Some(Url {
                            url,
                            version: Some(sanitize_string(version)),
                            after_version: None,
                        });
                    }
                }
            } else {
                return Some(Url {
                    url: format!("{url}/"),
                    version: Some(sanitize_string(version)),
                    after_version: None,
                });
            }
        }
        None
    }

    /// Only retains entries from `HttpDirectory` listing that
    /// does NOT match with any of the regular expressions found
    /// in `image_name_cleanse` field
    fn clean_httpdir_from_image_name_cleanse_regex(&self, image_list: HttpDirectory) -> HttpDirectory {
        debug!("[{}] Cleaning (length: {}): {image_list}", self.name, image_list.len());
        let mut filtered_image_list = image_list;
        if let Some(regex_list_to_remove) = &self.image_name_cleanse {
            for regex_to_remove in regex_list_to_remove {
                if let Ok(re) = Regex::new(regex_to_remove) {
                    debug!(" -> Using '{regex_to_remove}' as Regex");
                    filtered_image_list = filtered_image_list.filtering(|e| !e.is_match_by_name(&re));
                }
            }
        }
        debug!("[{}] Cleaned (length: {}): {filtered_image_list}", self.name, filtered_image_list.len());
        filtered_image_list
    }

    /// Adds the latest image that can be gathered from this `url`.
    /// Downloads through `client` connection if possible a checksum
    /// file and extracts the checksum. Returns a `Option<CloudImage>`
    /// that represents the latest downloadable image if any)
    ///
    /// # Errors
    /// May return an `HttpDirError` if getting the `HttpDirectory` for
    /// this url fails
    ///
    /// @todo: simplify
    async fn get_latest_image_to_download_from_url(
        &self,
        url: &Url,
        client: &reqwest::Client,
        db: &DbImageHistory,
    ) -> Result<Option<CloudImage>, HttpDirError> {
        let mut option_cloud_image: Option<CloudImage> = None;

        // Getting all files whose name matches the regex self.image_name_filter and
        // that does not matches *any* of the cimage_name_cleanse regex vector entry
        let url_httpdir = HttpDirectory::new(&url.url).await?;
        let http_image_list = url_httpdir.files().filter_by_name(&self.image_name_filter)?;
        debug!(
            "[{}] Retrieved {} files filtered with '{}' filter from {}",
            self.name,
            http_image_list.len(),
            self.image_name_filter,
            url.url
        );
        let http_image_list = self.clean_httpdir_from_image_name_cleanse_regex(http_image_list);

        // Keeping only the newest entry from that list
        if let Some(image) = http_image_list.sort_by_date(Sorting::Descending).first()
            && let Some(image_name) = image.name()
            && let Some(date) = image.date()
        {
            // Trying to find if we have a file that contains all checksums for
            // the files to be downloaded
            let one_file = url_httpdir.files().filtering(|e| {
                are_all_checksums_in_one_file(e.name().expect(
                    ".files() filter should return only files with names and thus .name() should never be None",
                ))
            });
            let one_file_count = one_file.len();
            debug!("Checksum guess: all in one file: {one_file_count}");
            // We choose to download only one file if possible: we test onefile
            // at first for this

            if one_file_count == 1 {
                // We only have one file with all checksums so get it:
                if let Some(checksum_entry) = one_file.first() {
                    // Download the checksum file with filename (url/filename)
                    // retrieving the image name's checksum from that file.
                    if let Some(filename) = checksum_entry.name() {
                        // downloading the checksum file
                        let checksums = get_body_from_url(&format!("{}/{filename}", url.url), client).await;
                        trace!("checksums: {checksums:?}");
                        // Finds the image_name in the checksum list and get it's checksum if any
                        let checksum =
                            CheckSums::get_image_checksum_from_checksums_buffer(image_name, &checksums, filename);
                        let new_url = Url {
                            url: format!("{}/{image_name}", url.url),
                            version: url.version.clone(),
                            after_version: url.after_version.clone(),
                        };
                        option_cloud_image = Some(CloudImage::new(new_url, checksum, image_name.to_string(), date));
                    }
                }
            } else {
                // We know that ".SHA256SUM" is a correct Regex so filter_by_name should never
                // return an Error here
                let everyfile = url_httpdir
                    .files()
                    .filter_by_name(".SHA256SUM")
                    .expect(".files() filter should return only files with names and thus .name() should never be None")
                    .len();
                if everyfile >= 1 {
                    // Downloading a checksum file that contains only the checksums of the image file
                    let checksum_filename = format!("{}.SHA256SUM", url.url);
                    let checksum_body = get_body_from_url(&checksum_filename, client).await;
                    let checksum =
                        CheckSums::get_image_checksum_from_checksums_buffer(image_name, &checksum_body, &url.url);
                    let new_url = Url {
                        url: url.url.clone(),
                        version: url.version.clone(),
                        after_version: url.after_version.clone(),
                    };
                    option_cloud_image = Some(CloudImage::new(new_url, checksum, image_name.to_string(), date));
                } else {
                    let new_url = Url {
                        url: format!("{}/{image_name}", url.url),
                        version: url.version.clone(),
                        after_version: url.after_version.clone(),
                    };
                    option_cloud_image = Some(CloudImage::new(new_url, CheckSums::None, image_name.to_string(), date));
                }
            }
        }

        if let Some(cloud_image) = option_cloud_image {
            if cloud_image.is_in_db(db) {
                warn!("Image {} is already in database", cloud_image.url.url);
                Ok(None)
            } else {
                info!("Image {} is not already in database", cloud_image.url.url);
                Ok(Some(cloud_image))
            }
        } else {
            Ok(None)
        }
    }
}

impl WSImageList {
    /// Retrieves for this website all downloadable images and makes an
    /// `ImageList` (ie an image url and an associated checksum).
    /// Returns a `WSImageList` formed with the website itself and a vector of
    /// `CloudImage`
    pub async fn get_images_list(website: Arc<WebSite>, concurrent_downloads: usize, db: Arc<DbImageHistory>) -> Self {
        // Creates a reqwest client to fetch url with.
        let client = reqwest::Client::new();

        // Generate a list of all url to be checked upon the
        // configuration and how is organized the website
        // itself (ie with or without dates directories)
        let url_list = website.generate_url_list().await;

        // Doing I/O get reqwest has much as possible in
        // a parallel way
        let lists = stream::iter(url_list)
            .map(|url| {
                let client = &client;
                let website = website.clone();
                let db = db.clone();
                async move {
                    match website.get_latest_image_to_download_from_url(&url, client, &db).await {
                        Ok(cloud_image) => cloud_image,
                        Err(error) => {
                            error!("[{}] Error with url ({}) retrieving image list: {error}", website.name, url.url);
                            None
                        }
                    }
                }
            })
            .buffered(concurrent_downloads);

        let all_cloud_images: Vec<Option<CloudImage>> = lists.collect().await;
        debug!("[{}] Number of images: {}", website.name, all_cloud_images.len());
        let images_list: Vec<CloudImage> = all_cloud_images.into_iter().flatten().collect();

        /*
                for option_cloud_image in all_cloud_images {
                    if let Some(cloud_image) = option_cloud_image {
                        images_list.push(cloud_image);
                    }
                }
        */
        WSImageList {
            images_list,
            website,
        }
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.images_list.is_empty()
    }
}

/// Returns true only if all `WSImageList` contained in the vector
/// `all_ws_image_list` are empty. Returns false otherwise
#[must_use]
pub fn vec_ws_image_lists_is_empty(all_ws_image_lists: &Vec<WSImageList>) -> bool {
    let mut is_empty = true;
    for ws_image in all_ws_image_lists {
        is_empty = is_empty && ws_image.is_empty();
    }
    is_empty
}

/// Returns an url formed with the last directory name found
/// in the `list_of_entries` if any.
fn url_with_latest_directory_name(name: &str, list_of_entries: HttpDirectory, url: &str) -> Option<(String, String)> {
    if let Some(entry) = list_of_entries.sort_by_name(Sorting::Descending).first() {
        if let Some(dirname) = entry.dirname() {
            debug!("[{name}] Adding {dirname}");
            Some((format!("{url}/{dirname}"), sanitize_string(dirname)))
        } else {
            debug!("[{name}] Error getting directory name");
            None
        }
    } else {
        debug!("[{name}] Error while trying to get the latest directory entry");
        None
    }
}
