extern crate md5;
extern crate docopt;
extern crate rustc_serialize;
extern crate yaml_rust;
extern crate regex;
extern crate hyper;

use docopt::Docopt;
use std::fs::{File, DirBuilder, OpenOptions};
use std::io::prelude::*;
use std::io::BufReader;
use yaml_rust::{YamlLoader, Yaml};
use yaml_rust::yaml::Hash;
use std::path::{Path, PathBuf};
use hyper::client::Client;
use hyper::header::{Headers, Authorization, Basic};
use hyper::Url;

// Write the Docopt usage string.
const USAGE: &'static str = "
TopSolid Update Downloader.

Usage:
  tsupdate <path> <version> [options]
  tsupdate (-h | --help)

Options:
  -h --help  Show this screen.
  -c FILE --cred=FILE  Cred file [default: credentials.yaml]
  -r ARCH --arch=ARCH  Architecture to download [default: x64]
  -a LEVEL --access=LEVEL  Access Level [default: Com]
";

#[derive(RustcDecodable)]
#[derive(Debug)]
enum Arch {
    X64,
    X86,
}

impl Arch {
    fn as_text<'a>(self: &Arch) -> &'a str {
        match self {
            &Arch::X64 => "x64",
            &Arch::X86 => "x86",
        }
    }
}
#[derive(RustcDecodable)]
enum Level {
    Com,
    Test,
    Dev,
}

impl Level {
    fn as_text<'a>(self: &Level) -> &'a str {
        match self {
            &Level::Test => "Test",
            &Level::Dev => "Dev",
            &Level::Com => "Com",
        }
    }
}
#[derive(RustcDecodable)]
struct Args {
    arg_path: String,
    arg_version: String,
    flag_cred: String,
    flag_arch: Arch,
    flag_access: Level,
}

struct Credentials {
    user: String,
    password: String,
    website: String,
    master_file: String,
    tag_file: String,
}

fn parse_args() -> Args {
    let args: Args =
        Docopt::new(USAGE).and_then(|d| d.help(true).decode()).unwrap_or_else(|e| e.exit());
    args
}

fn get_value_from_hash(hash: &Hash, name: &str) -> String {
    hash.get(&Yaml::from_str(name)).unwrap().as_str().unwrap().to_string()
}

fn get_credentials(args: &Args) -> Option<Credentials> {
    let mut buf = String::new();
    let f = File::open(&args.flag_cred);
    match f {
        Ok(mut f) => {
            f.read_to_string(&mut buf).unwrap();
            let y = &YamlLoader::load_from_str(&buf).unwrap()[0];
            let hash = y.as_hash().unwrap();
            Some(Credentials {
                user: get_value_from_hash(hash, "user"),
                password: get_value_from_hash(hash, "password"),
                website: get_value_from_hash(hash, "website"),
                master_file: get_value_from_hash(hash, "master_file"),
                tag_file: get_value_from_hash(hash, "tag_file"),
            })
        }
        Err(_) => None,
    }
}

#[derive(Debug)]
struct UpdateItem {
    filename: String,
    size: u32,
    product: String,
    digest: String,
    downloaded: bool,
    checked: bool,
}

impl PartialEq for UpdateItem {
    fn eq(&self, other: &UpdateItem) -> bool {
        self.filename == other.filename && self.size == other.size &&
        self.product == other.product && self.digest == other.digest
    }
}

fn make_update_item(args: &Args,
                    line: std::io::Result<String>,
                    reg: &regex::Regex)
                    -> Option<UpdateItem> {
    let access_text = args.flag_access.as_text();
    let arch_text = args.flag_arch.as_text();
    match line {
        Ok(line) => {
            match reg.captures(&line) {
                Some(capt) => {
                    let capt_2 = capt.get(2);
                    let mut access_ok = true;
                    if capt_2.is_some() {
                        access_ok = capt_2.unwrap().as_str() == access_text;
                    }
                    if access_ok && args.arg_version == capt.get(3).unwrap().as_str() &&
                       arch_text == capt.get(4).unwrap().as_str() {
                        Some(UpdateItem {
                            product: capt.get(5).unwrap().as_str().to_string(),
                            filename: capt.get(6).unwrap().as_str().to_string(),
                            size: capt.get(7).unwrap().as_str().parse::<u32>().unwrap(),
                            digest: capt.get(9).unwrap().as_str().to_uppercase().to_string(),
                            downloaded: false,
                            checked: false,
                        })
                    } else {
                        None
                    }
                }
                None => None,
            }
        }
        Err(_) => None,
    }
}

fn read_present_items(args: &Args, creds: &Credentials, reg: &regex::Regex) -> Vec<UpdateItem> {
    let mut pitems: Vec<UpdateItem> = vec![];
    let input = File::open(Path::new(&args.arg_path).join(&creds.master_file));
    if input.is_ok() {
        let reader = BufReader::new(input.unwrap());
        for line in reader.lines() {
            match make_update_item(args, line, reg) {
                Some(item) => pitems.push(item),
                None => {}
            }
        }
    }
    pitems
}

fn read_web_items(args: &Args,
                  creds: &Credentials,
                  reg: &regex::Regex,
                  client: &Client)
                  -> Vec<UpdateItem> {
    let mut pitems: Vec<UpdateItem> = vec![];
    let mut auth_headers = Headers::new();
    auth_headers.set(Authorization(Basic {
        username: creds.user.to_owned(),
        password: Some(creds.password.to_owned()),
    }));

    match Url::parse(&creds.website) {
        Ok(url) => {
            let master = url.join(&creds.master_file).unwrap();
            match client.get(master).headers(auth_headers).send() {
                Ok(res) => {
                    let reader = BufReader::new(res);
                    for line in reader.lines() {
                        match make_update_item(args, line, reg) {
                            Some(ui) => pitems.push(ui),
                            None => {}
                        }
                    }
                }
                Err(_) => println!("Could not load master file from website."),
            }
        }
        Err(_) => println!("Website in credentials file is not a valid URL."),
    }
    pitems
}

fn download_patches(args: &Args,
                    creds: &Credentials,
                    disk: &Vec<UpdateItem>,
                    web: &mut Vec<UpdateItem>,
                    client: &Client) {

    for ui in web.iter_mut() {
        if !disk.iter().any(|u| u == ui) {
            println!("Downloading {}", ui.filename);
            let url = format!("{}/{}/{}/{}/{}",
                              &creds.website,
                              &args.arg_version,
                              args.flag_arch.as_text(),
                              &ui.product,
                              &ui.filename);
            let mut auth_headers = Headers::new();
            auth_headers.set(Authorization(Basic {
                username: creds.user.to_owned(),
                password: Some(creds.password.to_owned()),
            }));
            match client.get(&url).headers(auth_headers).send() {
                Ok(mut res) => {
                    match File::create(compute_path(args, ui, true)) {
                        Ok(mut outf) => {
                            match std::io::copy(&mut res, &mut outf) {
                                Ok(_) => ui.downloaded = true,
                                Err(_) => println!("An error occurred when downloading!"),
                            }
                        }
                        Err(e) => println!("Could not create output file. {:?}", e),
                    }
                }
                Err(_) => println!("Could not download {}", ui.filename),
            }

        }

    }
}

fn compute_path(args: &Args, ui: &UpdateItem, create_dir: bool) -> PathBuf {
    let mut filepath = PathBuf::from(&args.arg_path);
    filepath.push(&args.arg_version);
    filepath.push(args.flag_arch.as_text());
    filepath.push(&ui.product);

    if !filepath.exists() && create_dir {
        DirBuilder::new().recursive(true).create(&filepath).unwrap();
    }
    filepath.push(&ui.filename);
    filepath
}

fn check_md5_sums(args: &Args, web: &mut Vec<UpdateItem>) {
    for ui in web.iter_mut().filter(|u| u.downloaded) {
        let path = compute_path(args, ui, false);
        match File::open(&path) {
            Ok(mut inf) => {
                let mut md5_con = md5::Context::new();
                match std::io::copy(&mut inf, &mut md5_con) {
                    Ok(_) => {
                        let form_digest = format!("{:X}", md5_con.compute());
                        ui.checked =  form_digest == ui.digest;
                        if !ui.checked {
                            std::fs::remove_file(path).unwrap();
                        }
                    }
                    Err(_) => {}
                }
            }
            Err(_) => {}
        }
    }
}

fn update_master_file(args: &Args, creds: &Credentials, web: &Vec<UpdateItem>) {
    let local_master = PathBuf::from(&args.arg_path).join(&creds.master_file);
    let access_text = args.flag_access.as_text();
    let arch_text = args.flag_arch.as_text();
    match OpenOptions::new().append(true).open(&local_master) {
        Ok(mut fi) => {
            for ui in web.iter().filter(|u| u.checked) {
                write!(&mut fi,
                       "{} {}/{}/{}/{} {} bytes MD5:{}\n",
                       access_text,
                       args.arg_version,
                       arch_text,
                       ui.product,
                       ui.filename,
                       ui.size,
                       ui.digest)
                    .unwrap();
            }
        }
        Err(_) => println!("Could not write Master file"),
    }
    let tag_file = PathBuf::from(&args.arg_path).join(&creds.tag_file);
    if !tag_file.exists() {
        File::create(tag_file).unwrap();
    }
}

fn main() {
    let args = parse_args();
    let creds = get_credentials(&args);
    match creds {
        Some(creds) => {
            let reg = regex::Regex::new(r"((Com|Dev|Test)[\s-]+)?([0-9\.]+)/(x86|x64)/([\w'\s]+)/([^/]+)\s(\d+)\sbytes(\sMD5:([0-9A-Fa-f]+))?$").unwrap();
            let items_from_disk = read_present_items(&args, &creds, &reg);
            if !items_from_disk.is_empty() {
                let client = Client::new();
                let mut items_from_web = read_web_items(&args, &creds, &reg, &client);
                if !items_from_web.is_empty() {
                    download_patches(&args,
                                     &creds,
                                     &items_from_disk,
                                     &mut items_from_web,
                                     &client);
                    check_md5_sums(&args, &mut items_from_web);
                    update_master_file(&args, &creds, &items_from_web)
                }
            }
        }
        None => println!("Credentials file {} could not be found.", args.flag_cred),
    }
}
