use cloud_image_download::CONCURRENT_REQUESTS;
use cloud_image_download::checksums::CheckSums;
use cloud_image_download::cli::Cli;
use cloud_image_download::download::download_images;
use cloud_image_download::image_history::DbImageHistory;
//use cloud_image_download::image_list::ImageList;
use cloud_image_download::settings::Settings;
use cloud_image_download::website::WSImageList;
use directories::BaseDirs;
use env_logger::{Env, WriteStyle};
use futures::{StreamExt, stream};
use log::{debug, error};
use std::env::var;
use std::process::exit;
use std::sync::Arc;

///  `NO_COLOR` compliance: See [no color web site](https://no-color.org/)
fn get_no_color_compliance_writestyle() -> WriteStyle {
    if var("NO_COLOR").is_ok() {
        WriteStyle::Never
    } else {
        WriteStyle::Auto
    }
}

/// Initializes logging environment with `NO_COLOR` compliance
fn init_log_environment(cli: &Cli) {
    let color = get_no_color_compliance_writestyle();

    // Retrieves verbosity level set at the cli level with -v, -vv or -q thanks to clap_verbosity
    let cli_debug_level = cli.verbose.log_level_filter().as_str();

    env_logger::Builder::from_env(Env::default().default_filter_or(cli_debug_level)).write_style(color).init();
}

#[tokio::main]
async fn main() {
    let cli = Cli::analyze();
    init_log_environment(&cli);

    let settings = Settings::from_config(&cli);
    debug!("Settings: {settings:?}");

    let base_dirs = match BaseDirs::new() {
        Some(b) => b,
        None => {
            error!("Unable to get $HOME directory");
            exit(1);
        }
    };

    let db = DbImageHistory::open(base_dirs.cache_dir().join("cid.sqlite"));
    db.create_db_image_history();

    let ws_image_list = stream::iter(settings.sites)
        .map(|website| async move { WSImageList::get_images_url_list(Arc::new(website)).await })
        .buffered(CONCURRENT_REQUESTS);

    let all_ws_image_lists = ws_image_list.collect::<Vec<WSImageList>>().await;

    download_images(all_ws_image_lists, &cli.verbose, cli.concurrent_downloads).await;
}
