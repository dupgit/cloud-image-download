Downloads cloud images from configured directories to some locations

One may read [architecture documentation](docs/architecture.md) to know
project's architecture.

# Configuration

Create a [TOML](https://toml.io/en/) configuration file:

`db_path` tells where to store the sqlite database (default
is `~/.cache/cid.sqlite`).

Use `[[sites]]` to define a site as an array of sites
from whom to download cloud images. Each site may have
the following keys:
- `name` as a string to name the site,
- `version_list` as an array of versions to look for,
- `base_url` as a string that tells cloud-image-download
   the base url where to download images,
- `after_version_url` as an array of directories to be added
  after the version (from `version_list`), `image_name_filter`
  as a Regex string to filter only these image names to download,
- `image_name_cleanse` as an array of Regex string to filter
  out the images (the images to download should not match
  any of those strings),
- `normalize` is a string that will be used as a template to
  normalize the filename of the saved image. You can use
  `{version}`, `{date}` and `{after_version}` modifiers in this
  string to customize the image name after these variable.
- `destination` is a directory where to store the downloaded
  images.

[test_data/cloud-image-download.toml] is a configuration
file example used for testing
