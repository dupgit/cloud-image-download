# cloud-image-download architecture document

## Introduction

### Program's goal

Downloads cloud system images from web sites and keeps track of the images
already downloaded and thus download only the latest ones. It should be
used periodically (ie: weekly)

### Target audience

This program is to be used by cloud system administrators that needs to get
cloud images and put them into their cloud system in order for them to be
available to their cloud users.

### Context

This program has been written to download the images from a machine that has
access to the internet and provide them onto a shared file system to an
another program that uploads them to the local cloud where internet is not
available.


## Architectural overview

### High level diagram

![High level diagram](diagram.png)

### Component description

- **Command line interface**: All options that a user can provide from the
  command line.
- **Settings reader and aggregator**: Reads configuration file and gives a
  structure that aggregates all settings from command line, configuration
  file, environment variable. Uses config crate.
- **Image history**: manages a structure with the history of all
  files that were previously successfully downloaded. Loads and saves
  image history from an sqlite DB. Uses crate rusqlite.
- **Website qualifier**: guess from what type of website we need to
  download the image from to be able to get the checksum file.
- **Image list creator**: Creates a list of all images that are available
  to download.
- **Image list filtering**: Filters the list of images based on criteria.
  Default criteria is that the file hasn't already been downloaded. This
  may be changed upon with command line parameters.
- **File downloader**:
  Downloads effectively all the images that needs to be downloaded. Uses
  crate trauma.
- **File checksum verifier**:
  Downloads the checskum files and verifies it. Uses crate sha2.


## Component overwiew

### Modules

#### Command line interface

Options a user can provide to the program. These option will only affect
the program's behavior:

- verbosity level,
- configuration file,
- Database path,


The command line will not collect parameters to download images from a
specific web site.

#### Settings reader and aggregator

Reads configuration file and aggregates it with parameters from the
command line, variable environment. These settings may be:

- Program settings
  - Proxy settings
  - Database path
- Web sites list
  - Name of the web site
  - Base url where to find the images
  - List of versions (of the images to dowload)
  - Web site type (Normal, WithDate)
  - List of complementary url (one may want more than one architecture
    for instance)
  - Image name (regular expression to be able to find it's name)
  - Destination path (where a dowloaded image will be saved)

#### Image history

Keeps names of successfully downloaded images in a database:

- The database should be created if it does not exist already,
- This module provides a filter in order to tell if an image,
  is or isn't already in the database. To be able to build a
  list of images to be downloaded from a list of potential images,
- When a downloaded image has been successfully verified (with its
  checksum) the image name and its date is saved into the database.
  The checksum verification is left to the checksum verifier module

#### Website qualifier

Guesses what type of checksum file we might download to get the
checksum of the file (looking at SHA256 checksums). It should return
a type:

- OneFile if all checksums are in one checksum file
- EveryFile if each image file as an associated checksum file

#### Image list creator

Gets from each web site a list of all files that may be downloaded.

#### Image list filtering

Filters the image list with the help of the database

#### File downloader

Downloads all files that are to be downloaded into it's final
destination configuration.

Downloads the checksum files into a temporary destination.

#### File checksum verifier

Calculates the checksum of a downloaded file and checks it against
the one from the downloaded checksum file.

#### Proxy settings

Get proxy settings from environment variables. Uses serde and
Deserialize derive functionality to retrieve parameters from
the configuration file


### Dependencies

- **clap**: The defacto standard for command line parsing argument
- **clap-verbosity**: Manages -v (--verbose) or -q (--quiet) options along
  with the log system.
- **config**: Reading configuration from files and environment variables
- **directories**: to get user directories from XDG specifications [1]
- **reqwest**: Interface to http requests helps revrieving web pages
- **rusqlite**: Gives access to an sqlite database and is used to store the
  download history
- **scraper**: Parses HTML. Used to extract all links from a web page
- **serde**: Serialization / Deserialization library used to read the
  configuration file into a dedicated structure.
- **sha2**: Widely use library to process sha checksums and all checksums out
  there for cloud images are SHA-256 ones.
- **shellexpand**: expands paths filenames with `~` or variables such as
  `${USER}` or `${HOME}`.
- **trauma**: Downloads files and seems more maintained than downloader crate.

## References

1. [XDG specifications](https://specifications.freedesktop.org/basedir-spec/latest/)
