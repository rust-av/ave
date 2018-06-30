#[macro_use]
extern crate structopt;

// Core crates
extern crate av_data as data;
extern crate av_codec as codec;
extern crate av_format as format;

// TODO: move those dependencies to av-formats
// Demuxers
extern crate matroska;

// TODO: move those dependencies to av-codecs
// Codecs
extern crate libvpx as vpx;
extern crate libopus as opus;
extern crate av_vorbis as vorbis;

// Command line interface
use std::path::{PathBuf, Path};
use std::fs::File;
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

use data::frame::ArcFrame;
use codec::common::CodecList;

use codec::decoder::Context as DecoderCtx;
use codec::decoder::Codecs as Decoders;

use format::demuxer::Context as DemuxerCtx;
use format::demuxer::Event;
use format::buffer::AccReader;

use vpx::decoder::VP9_DESCR as VP9_DEC;
use opus::decoder::OPUS_DESCR as OPUS_DEC;
use vorbis::decoder::VORBIS_DESCR as VORBIS_DEC;

use matroska::demuxer::MkvDemuxer;

use std::collections::HashMap;

/// The source binds a single Demuxer
/// to as many Decoders as the Streams
struct Source {
    decoders: HashMap<usize, DecoderCtx>,
    demuxer: DemuxerCtx,
}

impl Source {
    /// Creates a source from a path
    // TODO:
    // - use multiple demuxers
    // - make the codec list allocation external
    fn from_path<P: AsRef<Path>>(path: P) -> Self {
        let decoder_list = Decoders::from_list(&[VP9_DEC, OPUS_DEC,
            VORBIS_DEC]);

        let r = File::open(path).unwrap();
        let ar = AccReader::with_capacity(4 * 1024, r);
        let mut demuxer = DemuxerCtx::new(Box::new(MkvDemuxer::new()), Box::new(ar));
        demuxer.read_headers().expect("Cannot parse the format headers");

        let mut decoders: HashMap<usize, DecoderCtx> =
            HashMap::with_capacity(demuxer.info.streams.len());

        for st in &demuxer.info.streams {
            if let Some(ref codec_id) = st.params.codec_id {
                if let Some(mut ctx) = DecoderCtx::by_name(&decoder_list, codec_id) {
                    if let Some(ref extradata) = st.params.extradata {
                        ctx.set_extradata(extradata);
                    }
                    ctx.configure().expect("Codec configure failed");
                    decoders.insert(st.index, ctx);
                    info!("Registering {} for stream {} (id {})", codec_id, st.index, st.id);
                }
            }
        }

        Source {
            decoders,
            demuxer,
        }
    }

    fn decode_one(&mut self) -> Result<Option<ArcFrame>, String> {
        let ref mut c = self.demuxer;
        let ref mut decs = self.decoders;
        match c.read_event() {
            Ok(event) => match event {
                Event::NewPacket(pkt) => {
                    if pkt.stream_index >= 0 {
                        let idx = pkt.stream_index as usize;
                        if let Some(dec) = decs.get_mut(&idx) {
                            debug!("Decoding packet at index {}", pkt.stream_index);
                            // TODO report error
                            dec.send_packet(&pkt).unwrap();
                            Ok(dec.receive_frame().ok())
                        } else {
                            debug!("Skipping packet at index {}", pkt.stream_index);
                            Ok(None)
                        }
                    } else {
                        warn!("Spurious packet");
                        Ok(None)
                    }
                },
                _ => {
                    error!("Unsupported event {:?}", event);
                    unimplemented!();
                }
            },
            Err(err) => {
                warn!("No more events {:?}", err);
                Err("TBD".to_owned())
            }
        }
    }
}

use log::LevelFilter;
use pretty_env_logger::formatted_builder;

use std::thread;
// use std::sync::mpsc;

fn main() {
    let mut builder = formatted_builder().unwrap();
    builder.filter_level(LevelFilter::Debug).init();

    let opt = Opt::from_args();

    let mut src = Source::from_path(&opt.input);

    let th_src = thread::spawn(move || {
        while let Ok(res) = src.decode_one() {
            info!("Decoded {:#?}", res);
        }
    });

    let _ = th_src.join();
}
