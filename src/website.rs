use crate::settings::Settings;
use log::debug;
use reqwest::get;
use scraper::{Html, Selector};
use std::process::exit;
use colored::Colorize;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
enum WebSiteType {
    VersionListWithDate,
    VersionList,
}

#[derive(Debug, Deserialize)]
pub struct VersionList {
    list: Vec<String>,
}

impl VersionList {
    fn default() -> Self {
        VersionList {
            list: Vec::default(),
        }
    }
    fn new() -> Self {
        Self::default()
    }

    fn push(&mut self, version: &str) -> &mut Self {
        self.list.push(version.to_string());
        self
    }
}

#[derive(Debug, Deserialize)]
enum CheckSumType {
    OneFile,
    EveryFile,
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct WebSite {
    name: String,
    web_site_type: WebSiteType,
    version_list: VersionList,
    base_url: String,
    after_version_url: Option<String>,
    base_image_name: String,
}

impl WebSite {
    fn new(
        name: &str,
        web_site_type: WebSiteType,
        version_list: VersionList,
        base_url: &str,
        after_version_url: Option<String>,
        base_image_name: &str,
    ) -> Self {
        WebSite {
            name: name.to_string(),
            web_site_type,
            version_list,
            base_url: base_url.to_string(),
            after_version_url,
            base_image_name: base_image_name.to_string(),
        }
    }

    fn baseurl_upon_version(&self, version: &str) -> String {
        let baseurl = match &self.after_version_url {
            Some(after) => format!("{}/{}/{}/", self.base_url, version, after),
            None => format!("{}/{}/", self.base_url, version),
        };
        debug!("baseurl: {baseurl}");
        baseurl
    }

    fn guess_checksum_type(&self, document: Html, selector: Selector) -> CheckSumType {
        let mut everyfile: u16 = 0;
        let mut onefile: u16 = 0;

        for element in document.select(&selector) {
            let inner = element.inner_html();
            debug!("Checksum guess: inner: {inner}");
            if inner.contains("SHA256SUM") {
                everyfile += 1;
            }
            if inner.contains("CHECKSUM") || inner.contains("SHA256SUMS") {
                onefile += 1;
            }
        }
        debug!("Checksum guess: everyfile: {everyfile}, onefile: {onefile}");
        if everyfile > 1 {
            CheckSumType::EveryFile
        } else if onefile == 1 {
            CheckSumType::OneFile
        } else {
            CheckSumType::Unknown
        }
    }

    /// Returns true on inner element that we want to keep.
    /// every inner element that contains:
    ///  - base image name
    ///  - x86_64 only once
    ///  - qcow2
    /// but not the ones that contains:
    ///  - latest
    ///  - MD5SUM
    ///  - SHA1SUM
    ///  - SHA256SUM
    fn filter_element(&self, inner: &String) -> bool {
        if inner.contains(&self.base_image_name)
            && inner.match_indices("x86_64").collect::<Vec<_>>().len() == 1
            && inner.contains("qcow2")
            && !inner.contains("latest")
            && !inner.contains("MD5SUM")
            && !inner.contains("SHA1SUM")
            && !inner.contains("SHA256SUM")
        {
            debug!("{} {inner}", "🗸".green());
            true
        } else {
            debug!("{} {inner}", "𐄂".red());
            false
        }
    }

    async fn get_images_names(&self) -> Vec<String> {
        let mut list = Vec::new();

        match self.web_site_type {
            WebSiteType::VersionListWithDate => {}
            WebSiteType::VersionList => {
                for version in &self.version_list.list {
                    let baseurl = self.baseurl_upon_version(version);
                    if let Ok(response) = get(baseurl).await {
                        if let Ok(body) = response.text().await {
                            let document = Html::parse_document(&body);
                            // Selects all links
                            let selector = match Selector::parse("a") {
                                Ok(s) => s,
                                Err(e) => {
                                    println!("Error: {e}");
                                    exit(1);
                                }
                            };
                            // selecting all <a> html tag then mapping it's inner element
                            // then filtering these elements with filter_element() function
                            list.extend(
                                document
                                    .select(&selector)
                                    .map(|element| element.inner_html())
                                    .filter(|inner| self.filter_element(inner))
                                    .collect::<Vec<_>>(),
                            );

                            println!(
                                "{} ({}): {:?}",
                                self.name,
                                version,
                                self.guess_checksum_type(document, selector)
                            );
                        }
                    }
                }
            }
        }
        list
    }
}

fn print_images(list: Vec<String>) {
    for name in list {
        println!("{name}");
    }
}

pub async fn get_web_site(settings: &Settings) {
    for websiteconfig in &settings.sites {
        let images = websiteconfig.site.get_images_names();
        print_images(images.await);
    }
}
