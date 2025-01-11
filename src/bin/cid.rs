use cloud_image_download::website::get_web_site;
use cloud_image_download::cli::Cli;
use cloud_image_download::settings::Settings;
use env_logger::{Env, WriteStyle};
use std::env::var;

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

    get_web_site(&settings).await;
}
