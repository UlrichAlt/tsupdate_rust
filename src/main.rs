
extern crate docopt;
extern crate rustc_serialize;
extern crate yaml_rust;

use docopt::Docopt;
use std::fs::File;
use std::io::prelude::*;
use yaml_rust::{YamlLoader, Yaml};
use yaml_rust::yaml::Hash;

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
    fn as_text<'a>(self: Arch) -> &'a str {
        match self {
            Arch::X64 => "x64",
            Arch::X86 => "x86",
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
    fn as_text<'a>(self: Level) -> &'a str {
        match self {
            Level::Test => "Test",
            Level::Dev => "Dev",
            Level::Com => "Com",
        }
    }
}
#[derive(RustcDecodable)]
struct Args {
    arg_path: String,
    arg_version: String,
    flag_help: bool,
    flag_cred: String,
    flag_arch: Arch,
    flag_access: Level,
}

struct Credentials {
    realm: String,
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

fn get_credentials(args: &Args) -> Credentials {
    let mut buf = String::new();
    let mut f = File::open(args.flag_cred.as_str()).unwrap();
    f.read_to_string(&mut buf).unwrap();
    let y = &YamlLoader::load_from_str(buf.as_str()).unwrap()[0];
    let hash = y.as_hash().unwrap();

    Credentials {
        realm: get_value_from_hash(hash, "realm"),
        user: get_value_from_hash(hash, "user"),
        password: get_value_from_hash(hash, "password"),
        website: get_value_from_hash(hash, "website"),
        master_file: get_value_from_hash(hash, "master_file"),
        tag_file: get_value_from_hash(hash, "tag_file"),
    }
}

#[derive(Debug)]
struct UpdateItem {
    filename: String,
    size: u32,
    product: String,
}

fn main() {

    let args = parse_args();
    let creds = get_credentials(&args);


}
