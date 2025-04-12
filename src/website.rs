use crate::CID_USER_AGENT;
use crate::checksums::CheckSums;
use crate::download::image_has_been_downloaded;
use crate::image_history::DbImageHistory;
use crate::image_list::{CloudImage, ImageList, compare_str_by_date};
use colored::Colorize;
use futures::{StreamExt, stream};
use log::{debug, error, info, trace, warn};
use regex::Regex;
use reqwest::header::{ACCEPT, USER_AGENT};
use scraper::{Html, Selector};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::exit;
use std::sync::Arc;
use trauma::download::Summary;

/// Enum to tell the type of checksum
/// used by the website.
#[derive(Debug, Deserialize)]
enum CheckSumType {
    /// `CheckSumType::OneFile` is used for websites that have
    /// a recognized file that contains all checksums for all
    /// downloadable images
    OneFile {
        filename: String,
    },
    /// `CheckSumType::EveryFile` is used when no file has been
    /// recognized to get all the checksums but we have found
    /// evidence that the websites has checksum files for each
    /// image that one may download (We only look for SHA256
    /// checksum files)
    EveryFile,
    /// `CheckSumType::EveryFile` when none of the above has
    /// been found and we can not decide where are the checksums
    Unknown,
}

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

/// Tells whether inner String contains a date
/// formats in the wild are YYYYMMDD and YYYYMMDD-VVVV
/// where VVVV is a version number.
fn filter_dates(inner: &str) -> bool {
    let re = Regex::new(r"\d{8}(?:-\d{4})?/$").unwrap();
    re.is_match(inner)
}

/// Builds a Selector for all links (html <a> tag)
/// This should never fail so we exit the program
/// if it happens
fn build_all_link_selector() -> Selector {
    // Selects all links
    match Selector::parse("a") {
        Ok(s) => s,
        Err(e) => {
            error!("Error: {e}");
            exit(1);
        }
    }
}

/// Gets a list of all possible dates from one url
/// that may contain directories named with dates
/// in YYYYMMDD format. This uses an async get request
/// (from reqwest) to get the page at the url
async fn get_list_of_dates(url: &str) -> Vec<String> {
    let mut dates_list = vec![];
    match reqwest::get(url).await {
        Ok(response) => {
            match response.text().await {
                Ok(body) => {
                    let document = Html::parse_document(&body);
                    let selector = build_all_link_selector();
                    // selecting all <a> html tag then mapping it's inner element
                    // then filtering these elements with filter_dates() function
                    let dates_slash_list = document
                        .select(&selector)
                        .map(|element| element.inner_html())
                        .filter(|inner| filter_dates(inner))
                        .collect::<Vec<String>>();

                    for date_slash in dates_slash_list {
                        dates_list.push(date_slash.replace("/", ""))
                    }
                }
                Err(e) => warn!("Error: not body in response: {e}"),
            };
        }
        Err(e) => warn!("Error while fetching url {url}: {e}"),
    };

    // If any we only keep the latest date so we need to sort
    // the list and then keep only the last element
    dates_list.sort_by(|a, b| compare_str_by_date(a, b));
    let len = dates_list.len();
    if len >= 1 {
        dates_list = vec![dates_list.swap_remove(len - 1)];
    };

    dates_list
}

/// Tells if inner String indicates that we are
/// in presence of a checksum files that contains
/// all checksums for all downloadable images
fn are_all_checksums_in_one_file(inner: &String) -> bool {
    // -CHECKSUM is used in Fedora sites
    // CHECKSUM is used in Centos sites
    // SHA256SUMS is used in Ubuntu sites
    inner.contains("-CHECKSUM") || inner == "CHECKSUM" || inner == "SHA256SUMS" || inner == "SHA512SUMS"
}

/// Tells whether inner String may be a checksum file.
/// It can be a single checksum file or a multiple
/// checksum file
fn is_a_checksum_file(inner: &String) -> bool {
    let re = Regex::new(r"\.(SHA1|MD5|SHA256)SUM$").unwrap();
    are_all_checksums_in_one_file(inner) || re.is_match(inner)
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
    /// Generates all url to be checked for images for
    /// this particular website. Checks whether the site
    /// has dates directories and in that case adds them
    /// to the list. The returned list may be empty.
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
            let list_of_dates = get_list_of_dates(&url).await;
            if list_of_dates.is_empty() {
                final_url_list.push(url.to_string());
            } else {
                for date in list_of_dates {
                    final_url_list.push(format!("{url}/{date}"));
                }
            }
        }

        final_url_list
    }

    /// Adds all images that can be gathered from this
    /// `url` through `client` connection to a list and
    /// returns that list (which may be empty)
    /// @todo: simplify
    async fn add_images_from_url_to_images_list(
        &self,
        url: &String,
        client: &reqwest::Client,
        db: &DbImageHistory,
    ) -> ImageList {
        let mut images_url_list = ImageList::default();

        match get_body_from_url(url, client).await {
            Some(body) => {
                trace!("{body}");
                let document = Html::parse_document(&body);
                let selector = build_all_link_selector();

                // selecting all <a> html tag then mapping it's inner element
                // then filtering these elements with filter_element() function
                let image_list = document
                    .select(&selector)
                    .map(|element| element.inner_html())
                    .filter(|inner| self.filter_element(inner))
                    .collect::<Vec<_>>();

                match self.guess_checksum_type(&document, &selector, url) {
                    CheckSumType::OneFile {
                        filename,
                    } => {
                        // Download the CheckSum file with filename (url/filename)
                        // for each image_name in url_list build a list of
                        // image_name associated with it's Some(checksum) from
                        // list of checksums
                        let checksums = get_body_from_url(&format!("{url}/{filename}"), client).await;
                        trace!("checksums: {checksums:?}");
                        for image_name in image_list {
                            // Finds the image_name in the checksum list and get it's checksum if any
                            let checksum =
                                CheckSums::get_image_checksum_from_checksums_buffer(&image_name, &checksums, &filename);
                            let cloud_image = CloudImage::new(format!("{url}/{image_name}"), checksum);
                            images_url_list.push(cloud_image);
                        }
                    }
                    CheckSumType::EveryFile => {
                        // for each image_name in url_list download it's
                        // checksum file that ends with .SHA256SUM and associate
                        // the Some(checksum) with the image_name
                        for image_name in image_list {
                            let url = format!("{url}/{image_name}");
                            let checksum_filename = format!("{url}.SHA256SUM");
                            let checksum_body = get_body_from_url(&checksum_filename, client).await;
                            let checksum =
                                CheckSums::get_image_checksum_from_checksums_buffer(&image_name, &checksum_body, &url);

                            let cloud_image = CloudImage::new(url, checksum);
                            images_url_list.push(cloud_image);
                        }
                    }
                    CheckSumType::Unknown => {
                        // build the image_list and associate each image_name with None
                        for image_name in image_list {
                            let cloud_image = CloudImage::new(format!("{url}/{image_name}"), CheckSums::None);
                            images_url_list.push(cloud_image);
                        }
                    }
                };
            }
            None => (),
        };

        // If dates are in the image name than we need to filter them here
        // otherwise they have already been filtered out in `get_list_of_dates()`
        images_url_list.sort_by_date();
        images_url_list.only_keep_last_element();

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

    /// Guesses from the HTML document and the chosen selector (<a>)
    /// what type of checksum files we are dealing with. If anychoice
    /// can be made we choose the unique file (has it will minimise
    /// http get requests).
    /// url is only here for info!() logging
    fn guess_checksum_type(&self, document: &Html, selector: &Selector, url: &String) -> CheckSumType {
        let mut everyfile: u16 = 0;
        let mut onefile: u16 = 0;
        let mut filename = String::new();

        for element in document.select(selector) {
            let inner = element.inner_html();
            trace!("Checksum guess: inner: {inner}");
            if inner.contains(".SHA256SUM") {
                everyfile += 1;
            }
            if are_all_checksums_in_one_file(&inner) {
                filename = inner;
                onefile += 1;
            }
        }
        debug!("Checksum guess: everyfile: {everyfile}, onefile: {onefile}");
        // We choose to download only one file if possible: we test onefile
        // at first for this
        if onefile == 1 {
            info!("Guessed checksum type for {url}: OneFile ({filename})");
            CheckSumType::OneFile {
                filename,
            }
        } else if everyfile >= 1 {
            info!("Guessed checksum type for {url}: EveryFile");
            CheckSumType::EveryFile
        } else {
            info!("Guessed checksum type for {url}: Unknown");
            CheckSumType::Unknown
        }
    }

    /// Returns true on inner element that we want to keep:
    /// every inner element that matches the regular expression
    /// found in image_name_filter. As this regular expression
    /// comes from a user input we fail and exit in case of an
    /// error when building it. The matching image also needs
    /// not to be a checksum file.
    /// @todo: simplify
    fn filter_element(&self, inner: &String) -> bool {
        let mut is_filtered = match Regex::new(&self.image_name_filter) {
            Ok(re) => re.is_match(inner) && !is_a_checksum_file(inner),
            Err(e) => {
                error!("Error in regular expression ({}): {e}", self.image_name_filter);
                exit(1);
            }
        };

        if is_filtered {
            let mut cleaned = false;
            if let Some(cleanse_filters) = &self.image_name_cleanse {
                for clean_filter in cleanse_filters {
                    let is_cleaned = match Regex::new(clean_filter) {
                        Ok(re) => re.is_match(inner),
                        Err(e) => {
                            error!("Error in regular expression ({}): {e}", clean_filter);
                            exit(1);
                        }
                    };
                    debug!("cleaning {inner} with {clean_filter} led to {is_cleaned}");
                    cleaned = cleaned || is_cleaned;
                }
            }
            if cleaned {
                is_filtered = false;
                debug!("{} {inner}", "𐄂".red());
            } else {
                debug!("{} {inner}", "🗸".green());
            }
        } else {
            // This is really verbose so we want to print this only in trace level.
            trace!("{} {inner}", "𐄂".red());
        }
        is_filtered
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
    pub fn only_effectively_downloaded(all_ws_image_lists: &mut Vec<WSImageList>, downloaded_summary: &Vec<Summary>) {
        for ws_image in all_ws_image_lists {
            ws_image.images_list.list.retain(|cloud_image| {
                image_has_been_downloaded(downloaded_summary, &cloud_image.url, &ws_image.website.destination)
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
