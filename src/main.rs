#[macro_use]
extern crate clap;
use clap::App;
use error_chain::error_chain;
use reqwest::Client;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::{thread, time};
use tempfile::NamedTempFile;
use xdg::BaseDirectories;

error_chain! {
    foreign_links {
        Io(std::io::Error);
        HttpRequest(reqwest::Error);
    }
}

struct Config {
    addon_list: String,
    addon_dest: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    // Create config file if it doesn't exist
    // TODO: should be its own function?
    let xdg_dirs = BaseDirectories::with_prefix("mudcrab").unwrap();
    let conf_file = match xdg_dirs.find_config_file("config.toml") {
        Some(path) => path,
        None => {
            println!("Config file not found. Creating.");
            let newfile = create_conf_file(xdg_dirs).unwrap();
            println!("Config file created in {}", &newfile.to_str().unwrap());
            newfile
        }
    };

    println!("Using conf file: {}", conf_file.to_str().unwrap());

    let config = read_conf(&conf_file);
    // let config = Config {
    //     addon_list: config["addonList"].clone(),
    //     addon_dest: config["addonDest"].clone(),
    // };

    println!(
        "Using these conf: Addon List: {} - Destination: {}",
        config.addon_list, config.addon_dest
    );

    // Go through the addon list file
    let file = File::open(config.addon_list).expect("Failed opening addon list file: {}");
    let reader = BufReader::new(file);
    let client = Client::new();

    for (_, line) in reader.lines().enumerate() {
        // Go through it line by line
        let line = line.unwrap();
        // We only care about the addon number
        for n in line.split_whitespace().take(1) {
            let ziptemp = download_zip(n, &client).await?;
            unzip(&ziptemp, &config.addon_dest).await?;
            // be nice
            thread::sleep(time::Duration::from_millis(500));
        }
    }

    Ok(())
}

// create $XDG_CONFIG/mudcrab/config.toml
fn create_conf_file(xdg_dirs: BaseDirectories) -> Result<PathBuf> {
    let path_to_conf = xdg_dirs
        .place_config_file("config.toml")
        .expect("Can't create config dir");
    File::create(&path_to_conf)?;
    println!("Config file created in {}", &path_to_conf.to_str().unwrap());
    Ok(path_to_conf)
}

// read $XDG_CONFIG/mudcrab/config.toml and return a Config struct
// TODO: make it more OS-agnostic
// TODO: should probably be its own file like in
// https://github.com/mehcode/config-rs/tree/master/examples/hierarchical-env
fn read_conf(conf_file: &PathBuf) -> Config {
    let mut settings = config::Config::default();

    settings
        .merge(config::File::with_name(&conf_file.to_str().unwrap()))
        .expect("Merging conf file failed");

    Config {
        addon_list: settings
            .get_str("addonList")
            .expect("Config file must have an 'addonList' field"),
        addon_dest: settings
            .get_str("addonDir")
            .expect("Config file must have an 'addonDir' field"),
    }
}

// unzip tempfile into addon path
async fn unzip(ziptemp: &NamedTempFile, unzip_path: &str) -> Result<()> {
    let zipfile = ziptemp.as_file();
    let mut archive = zip::ZipArchive::new(zipfile).unwrap();
    let path = Path::new(&unzip_path);
    archive.extract(path).unwrap();

    Ok(())
}

async fn download_zip(number: &str, client: &Client) -> Result<NamedTempFile> {
    let url = format!("https://cdn.esoui.com/downloads/file{}/", number);
    let res = client.get(url).send().await?;

    let mut file = NamedTempFile::new()?;
    if res.status().is_success() {
        let content = res.bytes().await?;
        file.write(&content)?;
    }

    Ok(file)
}
