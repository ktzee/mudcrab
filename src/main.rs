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

error_chain! {
    foreign_links {
        Io(std::io::Error);
        HttpRequest(reqwest::Error);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let yaml = load_yaml!("cli.yml");
    let matches = App::from_yaml(yaml).get_matches();

    // Create config file if it doesn't exist
    // TODO: should be its own function?
    let xdg_dirs = xdg::BaseDirectories::with_prefix("mudcrab").unwrap();
    let conf_path = match xdg_dirs.find_config_file("config.toml") {
        Some(path) => path,
        None => {
            println!("Config file not found. Creating.");
            let x = xdg_dirs
                .place_config_file("config.toml")
                .expect("Can't create config dir");
            File::create(&x)?;
            println!("Config file created in {}", &x.to_str().unwrap());
            x
        }
    };

    println!("Using conf file: {}", conf_path.to_str().unwrap());

    let config = read_conf(&conf_path);

    println!("{:?}", config);

    // If we get optional command line arguments, use those over the config file
    let addon_list = match matches.value_of("FILE") {
        Some(arg) => {
            println!("FILE Command line argument provided. Using it.");
            arg
        }
        None => {
            println!("Using addon list in conf file.");
            if config.contains_key("addonListFile") {
                &config["addonListFile"]
            } else {
                panic!(
                    "add an addonListFile and addonDir to {}",
                    conf_path.to_str().unwrap()
                )
            }
        }
    };
    let addon_dest = match matches.value_of("DEST") {
        Some(arg) => {
            println!("DEST Command line argument provided. Using it.");
            arg
        }
        None => {
            println!("Using destination in conf file.");
            if config.contains_key("addonDir") {
                &config["addonDir"]
            } else {
                panic!(
                    "add an addonListFile and addonDir to {}",
                    conf_path.to_str().unwrap()
                )
            }
        }
    };

    println!(
        "Using these conf: Addon List: {} - Destination: {}",
        addon_list, addon_dest
    );

    // Go through the addon list file
    let file = File::open(addon_list).expect("Failed opening addon list file: {}");
    let reader = BufReader::new(file);
    let client = Client::new();

    for (_, line) in reader.lines().enumerate() {
        // Go through it line by line
        let line = line.unwrap();
        // We only care about the addon number
        for n in line.split_whitespace().take(1) {
            let ziptemp = download_zip(n, &client).await?;
            unzip(&ziptemp, &addon_dest).await?;
            // be nice
            thread::sleep(time::Duration::from_millis(500));
        }
    }

    Ok(())
}

// read $XDG_CONFIG/mudcrab/config.toml and return a HashMap of the config
// TODO: make it more OS-agnostic
// TODO: should probably be its own file like in
// https://github.com/mehcode/config-rs/tree/master/examples/hierarchical-env
fn read_conf(conf_path: &PathBuf) -> HashMap<String, String> {
    let mut settings = config::Config::default();

    settings
        .merge(config::File::with_name(&conf_path.to_str().unwrap()))
        .expect("Merging conf file failed");

    settings.try_into::<HashMap<String, String>>().unwrap()
}

// TODO: rework to use tempfile
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
