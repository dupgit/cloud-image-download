use crate::CID_USER_AGENT;
use crate::checksums::CheckSums;
use crate::download::image_has_been_downloaded;
use crate::image_history::DbImageHistory;
use crate::image_list::{CloudImage, ImageList};
use futures::{StreamExt, stream};
use httpdirectory::httpdirectory::{HttpDirectory, Sorting};
use log::{debug, info, trace, warn};
use regex::Regex;
use reqwest::header::{ACCEPT, USER_AGENT};
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use trauma::download::Summary;

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
}

/// Associates a list of images with the website
/// they come from
pub struct WSImageList {
    pub images_list: ImageList,
    pub website: Arc<WebSite>,
}

/// Tells if inner String indicates that we are
/// in presence of a checksum files that contains
/// all checksums for all downloadable images
fn are_all_checksums_in_one_file(inner: &str) -> bool {
    // -CHECKSUM is used in Fedora sites
    // CHECKSUM is used in Centos sites
    // SHA256SUMS is used in Ubuntu sites
    inner.contains("-CHECKSUM") || inner == "CHECKSUM" || inner == "SHA256SUMS" || inner == "SHA512SUMS"
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
                warn!("Error while retrieving url {url} content {}", response.status());
                None
            }
        },
        Err(e) => {
            warn!("Error while fetching url {url}: {e}");
            None
        }
    }
}

impl WebSite {
    /// Generates all url to be checked for images for this particular website
    /// using versions from `version_list` and `after_version_url` that both
    /// are vectors and may contain more than one element.
    /// Checks whether the site has dates directories and in that case adds
    /// the latest one to the list instead of the url itself.
    /// The returned list may be empty.
    async fn generate_url_list(&self) -> Vec<String> {
        let mut url_list = vec![];
        for version in &self.version_list {
            let version_list = match &self.after_version_url {
                Some(after) => {
                    let mut vl = vec![];
                    for after_version in after {
                        let url = format!("{}/{}/{}", self.base_url, version, after_version);
                        info!("Adding url '{url}' to list of url for {}", self.name);
                        vl.push(url);
                    }
                    vl
                }
                None => {
                    let url = format!("{}/{}", self.base_url, version);
                    info!("Adding url '{url}' to list of url for {}", self.name);
                    vec![url]
                }
            };
            url_list.extend(version_list);
        }

        let mut final_url_list = vec![];
        for url in url_list {
            if let Some(url_checked) = WebSite::check_for_directories_with_dates(&url).await {
                final_url_list.push(url_checked);
            }
        }
        final_url_list
    }

    /// Checks if an url has directories with dates and then returns the url
    /// containing that directory instead of url itself. If the url has no
    /// directories with dates then returns this url
    async fn check_for_directories_with_dates(url: &str) -> Option<String> {
        if let Ok(list_of_dates) = HttpDirectory::new(url).await {
            if let Ok(list_of_dates) = list_of_dates.dirs().filter_by_name(r"\d{8}(?:-\d{4})?/$") {
                if list_of_dates.is_empty() {
                    debug!("This url ({url}) has no dates in it");
                    return Some(url.to_string());
                } else {
                    debug!("This url ({url}) has dates in it:");
                    // Keep only the latest entry !
                    if let Some(entry) = list_of_dates.sort_by_date(Sorting::Descending).first() {
                        if let Some(date) = entry.dirname() {
                            debug!("Adding {date}");
                            return Some(format!("{url}/{date}"));
                        } else {
                            debug!("Error getting directory name");
                        }
                    } else {
                        debug!("Error while trying to get the latest directory entry");
                    }
                }
            } else {
                return Some(url.to_string());
            }
        }
        None
    }

    // Only retains entries from `HttpDirectory` listing that
    // does NOT matches with any of the regular expressions found
    // in `image_name_cleanse` field
    fn clean_httpdir_from_image_name_cleanse_regex(&self, image_list: HttpDirectory) -> HttpDirectory {
        debug!("Cleaning: {image_list}");
        let mut filtered_image_list = image_list;
        if let Some(regex_list_to_remove) = &self.image_name_cleanse {
            for regex_to_remove in regex_list_to_remove {
                if let Ok(re) = Regex::new(regex_to_remove) {
                    debug!(" -> Using '{regex_to_remove}' as Regex");
                    filtered_image_list = filtered_image_list.filtering(|e| !e.is_match_by_name(&re));
                }
            }
        }
        debug!("Cleaned: {filtered_image_list}");
        filtered_image_list
    }

    /// Adds all images that can be gathered from this
    /// `url`. Downloads through `client` connection if
    /// possible a checksum file and extracts the checksum.
    /// Returns an `ImageList` that contains either 0 or 1
    /// element
    /// @todo: simplify
    async fn add_images_from_url_to_images_list(
        &self,
        url: &String,
        client: &reqwest::Client,
        db: &DbImageHistory,
    ) -> ImageList {
        let mut images_url_list = ImageList::default();

        if let Ok(url_httpdir) = HttpDirectory::new(url).await {
            if let Ok(image_list) = url_httpdir.files().filter_by_name(&self.image_name_filter) {
                let image_list = self.clean_httpdir_from_image_name_cleanse_regex(image_list);
                // Keeping only the newest entry from that list
                if let Some(image) = image_list.sort_by_date(Sorting::Descending).first() {
                    if let Some(image_name) = image.name() {
                        // Trying to find if we have a file that contains all checksums for
                        // the files to be downloaded
                        let one_file = url_httpdir.files().filtering(|e| {
                            if let Some(name) = e.name() {
                                are_all_checksums_in_one_file(name)
                            } else {
                                false
                            }
                        });
                        let one_file_count = one_file.len();
                        debug!("Checksum guess: all in one file: {one_file_count}");
                        // We choose to download only one file if possible: we test onefile
                        // at first for this

                        if one_file_count == 1 {
                            if let Some(checksum_entry) = one_file.first() {
                                // Download the CheckSum file with filename (url/filename)
                                // for each image_name in url_list build a list of
                                // image_name associated with it's Some(checksum) from
                                // list of checksums
                                if let Some(filename) = checksum_entry.name() {
                                    // downloading the checksum file
                                    let checksums = get_body_from_url(&format!("{url}/{filename}"), client).await;
                                    trace!("checksums: {checksums:?}");
                                    // Finds the image_name in the checksum list and get it's checksum if any
                                    let checksum = CheckSums::get_image_checksum_from_checksums_buffer(
                                        image_name, &checksums, filename,
                                    );
                                    let cloud_image = CloudImage::new(format!("{url}/{image_name}"), checksum);
                                    images_url_list.push(cloud_image);
                                }
                            }
                        } else {
                            // We know that ".SHA256SUM" is a correct Regex so filter_by_name should never
                            // return an Error here
                            let everyfile = url_httpdir.files().filter_by_name(".SHA256SUM").unwrap().len();
                            if everyfile >= 1 {
                                let url = format!("{url}/{image_name}");
                                let checksum_filename = format!("{url}.SHA256SUM");
                                let checksum_body = get_body_from_url(&checksum_filename, client).await;
                                let checksum = CheckSums::get_image_checksum_from_checksums_buffer(
                                    image_name,
                                    &checksum_body,
                                    &url,
                                );

                                let cloud_image = CloudImage::new(url, checksum);
                                images_url_list.push(cloud_image);
                            } else {
                                let cloud_image = CloudImage::new(format!("{url}/{image_name}"), CheckSums::None);
                                images_url_list.push(cloud_image);
                            }
                        }
                    }
                }
            }
        }

        // Here we should have only one element or less in images_url_list

        // As we only have one element in the list (if any) we
        // can take the first one and test it against the database
        // if it is already in the database then we can return an
        // empty list has we already downloaded it.
        // @todo: may be add an option to allow one to reload again
        // an already successfully downloaded image ?
        if let Some(cloud_image) = images_url_list.list.first() {
            if cloud_image.is_in_db(db) {
                warn!("Image {} is already in database", cloud_image.url);
                images_url_list.list = vec![];
            } else {
                info!("Image {} is not already in database", cloud_image.url);
            }
        }

        images_url_list
    }
}

impl WSImageList {
    /// Retrieves for this website all downloadable images and makes an ImageList
    /// (ie an image url and an associated checksum).
    /// Returns a `WSImageList` formed with the website itself and the generated
    /// image list `Imagelist`.
    pub async fn get_images_url_list(
        website: Arc<WebSite>,
        concurrent_downloads: usize,
        db: Arc<DbImageHistory>,
    ) -> Self {
        let mut images_url_list = ImageList::default();
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
                async move { website.add_images_from_url_to_images_list(&url, client, &db).await }
            })
            .buffered(concurrent_downloads);

        let all_lists = lists.collect::<Vec<ImageList>>().await;

        for image_list in all_lists {
            images_url_list.extend(image_list);
        }

        WSImageList {
            website,
            images_list: images_url_list,
        }
    }

    /// Retains only images that have been effectively downloaded
    /// and not skipped, in error state or in "not started" state
    /// using `Vec<Summary>` to know their state
    pub fn only_effectively_downloaded(
        all_ws_image_lists: &mut Vec<WSImageList>,
        downloaded_summary: &Vec<Summary>,
        verify_skipped: bool,
    ) {
        for ws_image in all_ws_image_lists {
            ws_image.images_list.list.retain(|cloud_image| {
                image_has_been_downloaded(
                    downloaded_summary,
                    &cloud_image.url,
                    &ws_image.website.destination,
                    verify_skipped,
                )
            });
        }
    }

    pub fn is_empty(&self) -> bool {
        self.images_list.is_empty()
    }
}

pub fn vec_ws_image_lists_is_empty(all_ws_image_lists: &Vec<WSImageList>) -> bool {
    let mut is_empty = true;
    for ws_image in all_ws_image_lists {
        is_empty = is_empty && ws_image.is_empty();
    }
    is_empty
}
