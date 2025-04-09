use cloud_image_download::cli::Cli;
use cloud_image_download::download::{display_download_status_summary, download_images, verify_downloaded_file};
use cloud_image_download::image_history::DbImageHistory;
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

/// Initializes logging environment with `NO_COLOR` compliance
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
    let db = Arc::new(db);

    // Getting all images that should downloaded
    let ws_image_list = stream::iter(settings.sites)
        .map(|website| {
            let db = db.clone();
            async move { WSImageList::get_images_url_list(Arc::new(website), cli.concurrent_downloads, db).await }
        })
        .buffered(cli.concurrent_downloads);

    let mut all_ws_image_lists = ws_image_list.collect::<Vec<WSImageList>>().await;

    // Downloads images
    let downloaded_summary = download_images(&all_ws_image_lists, &cli.verbose, cli.concurrent_downloads).await;

    // This will only display a summary only if -q has not been selected
    display_download_status_summary(&downloaded_summary, &cli.verbose);

    // This will retain from `all_ws_image_lists` list only images
    // that have been effectively downloaded
    WSImageList::only_effectively_downloaded(&mut all_ws_image_lists, &downloaded_summary);

    verify_downloaded_file(all_ws_image_lists, db).await;
}
