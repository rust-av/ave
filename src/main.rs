#[macro_use]
extern crate structopt;

// Core crates
extern crate av_codec as codec;
extern crate av_data as data;
extern crate av_format as format;

// TODO: move those dependencies to av-formats
// Demuxers
extern crate matroska;

// TODO: move those dependencies to av-codecs
// Codecs
extern crate av_vorbis as vorbis;
extern crate libopus as opus;
extern crate libvpx as vpx;

// Command line interface
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "ave")]
/// Simple Audio Video Encoding tool
struct Opt {
    /// Input file
    #[structopt(short = "i", parse(from_os_str))]
    input: PathBuf,
    /// Output file
    #[structopt(short = "o", parse(from_os_str))]
    output: PathBuf,
}

// TODO: Use fern?
// Logging
extern crate pretty_env_logger;
#[macro_use]
extern crate log;

use format::common::GlobalInfo;

mod sink;
mod source;

use sink::*;
use source::*;

use log::LevelFilter;
use pretty_env_logger::formatted_builder;

use std::thread;
// use std::sync::mpsc;

fn main() {
    let mut builder = formatted_builder().unwrap();
    builder.filter_level(LevelFilter::Debug).init();

    let opt = Opt::from_args();

    let mut src = Source::from_path(&opt.input);

    let dummy_info = GlobalInfo {
        duration: None,
        timebase: None,
        streams: Vec::new(),
    };
    let mut sink = Sink::from_path(&opt.output, dummy_info);

    let th_src = thread::spawn(move || {
        while let Ok(res) = src.decode_one() {
            info!("Decoded {:#?}", res);
        }
    });

    let _ = th_src.join();
}
