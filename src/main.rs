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
use std::sync::mpsc;

use std::collections::HashMap;

use data::frame::ArcFrame;

use codec::encoder::Context as EncoderCtx;
use codec::common::CodecList;
use codec::encoder;

use format::stream::Stream;

use vpx::encoder::VP9_DESCR;
use opus::encoder::OPUS_DESCR;

fn main() {
    let mut builder = formatted_builder().unwrap();
    builder.filter_level(LevelFilter::Debug).init();

    let opt = Opt::from_args();

    let mut src = Source::from_path(&opt.input);

    let encoder_list = encoder::Codecs::from_list(&[VP9_DESCR, OPUS_DESCR]);

    let mut info = GlobalInfo {
        duration: src.demuxer.info.duration.clone(),
        timebase: src.demuxer.info.timebase.clone(),
        streams: Vec::new(),
    };

    info!("{:?}", src.demuxer.info);

    let encoders: Vec<EncoderCtx> = src.demuxer.info.streams.iter().scan(&mut info, |info, st| {
        // TODO: stream selection and mapping
        if let Some(ref codec_id) = st.params.codec_id {
            if let Some(mut ctx) = EncoderCtx::by_name(&encoder_list, codec_id) {
                // Derive a default setup from the input codec parameters
                debug!("Setting up {} encoder", codec_id);
                ctx.set_params(&st.params).unwrap();
                // Overrides here
                let _ = ctx.set_option("timebase", (*st.timebase.numer(), *st.timebase.denom()));
                ctx.configure().unwrap();
                info.add_stream(Stream::from_params(&ctx.get_params().unwrap(), st.timebase.clone()));
                Some(ctx)
            } else {
                None
            }
        } else {
            None
        }
    }).collect();

    info!("Encoders set {:?}", info);

    let mut sink = Sink::from_path(&opt.output, info);

    let th_src = thread::spawn(move || {
        while let Ok(res) = src.decode_one() {
            info!("Decoded {:#?}", res);
        }
    });

    let _ = th_src.join();
}
