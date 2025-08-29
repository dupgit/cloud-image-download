Downloads cloud images from configured directories to some locations

One may read [architecture documentation](docs/architecture.md) to know
project's architecture.

# Configuration

Create a [TOML](https://toml.io/en/) configuration file:

`db_path` tells where to store the sqlite database (default
is `~/.cache/cid.sqlite`).

Use `[[sites]]` to define a site as an array of sites
from whom to download cloud images. Each site may have
the following keys `name` as a String to name the site,
`version_list` as an array of versions to look for,
`base_url` as a String that tells cloud-image-download
the base url where to download images, `after_version_url`
as an array of directories to be added after the version
(from `version_list`), `image_name_filter` as a Regex string
to filter only these image names to download,
`image_name_cleanse` as an array of Regex string to filter
out the images (the images to download should not match
any of those strings), `normalize` is a boolean (`true`)
that tells cloud-image-download to add a date to the name
of the saved image and finally `destination` is a directory
where to store the downloaded images.

[test_data/cloud-image-download.toml] is a configuration
file example used for testing
