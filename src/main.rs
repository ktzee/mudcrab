use error_chain::error_chain;
use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::{thread, time};
use std::collections::HashMap;
use clap::{Arg, App,};
use reqwest::Client;


error_chain! {
    foreign_links {
        Io(std::io::Error);
        HttpRequest(reqwest::Error);
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let matches = App::new("Rusted Skeevatron")
                        .version("0.1")
                        .author("Giuseppe C. <giuseppe@ktz.one>")
                        .about("Downloads UI addons for TESO. 
                                Based on skeevatron.sh
                                (https://github.com/gangelop/skeevatron/blob/master/skeevatron.sh)
                                by George Angelopoulos <george@usermod.net>")
                        .arg(Arg::with_name("FILE")
                            .help("The addon list file. This overrides what's configured in your conf file.")
                            .required(false)
                            .index(1)
                            .short("l")
                            .long("list"))
                        .arg(Arg::with_name("DEST")
                            .help("The destination addon directory. This overrides what's configured in your conf file.")
                            .required(false)
                            .index(2)
                            .short("d")
                            .long("dest"))
                        .get_matches();

    // Create config file if it doesn't exist
    // TODO: should be its own function?
    let xdg_dirs = xdg::BaseDirectories::with_prefix("mudcrab").unwrap();
    let conf_path = match xdg_dirs.find_config_file("config.toml") {
        Some(path) => path,
        None => {
            println!("Config file not found. Creating.");
            let x = xdg_dirs.place_config_file("config.toml")
                .expect("Can't create config dir");
                File::create(&x)?;
            println!("Config file created in {}", &x.to_str().unwrap());
            x
        },
    };

    println!("Using conf file: {}", conf_path.to_str().unwrap());

    let config = read_conf(&conf_path);

    println!("{:?}", config);

    // If we get optional command line arguments, use those over the config file
    let addon_list = match matches.value_of("FILE") {
        Some(arg) => {
            println!("FILE Command line argument provided. Using it.");
            arg
        },
        None => {
            println!("Using addon list in conf file.");
            if config.contains_key("addonListFile"){
                &config["addonListFile"]
            } else {
                panic!("add an addonListFile and addonDir to {}", conf_path.to_str().unwrap())
            }
        }, 
    };
    let addon_dest = match matches.value_of("DEST") {
        Some(arg) => {
            println!("DEST Command line argument provided. Using it.");
            arg
        },
        None => {
            println!("Using destination in conf file.");
            if config.contains_key("addonDir"){
                &config["addonDir"]
            } else {
                panic!("add an addonListFile and addonDir to {}", conf_path.to_str().unwrap())
            }
        },
    };

    println!("Using these conf: Addon List: {} - Destination: {}", addon_list, addon_dest);


    // Go through the addon list file
    let file = File::open(addon_list)
        .expect("Failed opening addon list file: {}");
    let reader = BufReader::new(file);
    let client = Client::new();


    for (_, line) in reader.lines().enumerate() {
        // Go through it line by line
        let line = line.unwrap();
        // We only care about the addon number
        for n in line.split_whitespace().take(1) {
            let zipname = download_zip(n, &client).await?;
            unzip(zipname.as_str(), &addon_dest).await?;
            println!("Zip {} in {}", zipname, &addon_dest);
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

async fn unzip(zipname: &str, unzip_path: &str) -> Result<()> {
    let zip = File::open(&zipname).unwrap();
    let mut archive = zip::ZipArchive::new(zip).unwrap();
    let path = Path::new(&unzip_path);
    archive.extract(path).unwrap();
    
    Ok(())
}

async fn download_zip(number: &str, client: &Client) -> Result<String> {
    let url = format!("https://cdn.esoui.com/downloads/file{}/", number);
    let res = client.get(url).send().await?;
    let zip_dest = format!("/tmp/download{}.zip", number);
    let path = Path::new(&zip_dest);
    
    let mut file = match File::create(&path) {
        Err(why) => panic!("couldn't create file {}", why),
        Ok(file) => file,
    };

    if res.status().is_success() {
        let content = res.bytes().await?;
        file.write_all(&content)?;
        println!("written {}", zip_dest)
    }

    Ok(zip_dest)
}