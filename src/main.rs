#[macro_use]
extern crate structopt;

extern crate crossbeam_channel as channel;

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
use std::sync::Arc;

use data::frame::ArcFrame;
use data::packet::ArcPacket;

use codec::encoder::Context as EncoderCtx;
use codec::common::CodecList;
use codec::encoder;

use format::stream::Stream;

use vpx::encoder::VP9_DESCR;
use opus::encoder::OPUS_DESCR;

use channel as ch;


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

    // encoders -> muxer
    let (encoders, recv_packet): (Vec<thread::JoinHandle<_>>, ch::Receiver<ArcPacket>) = {
        let decoders = &mut src.decoders;
        let demuxer = &src.demuxer;
        let (send_packet, recv_packet) = ch::unbounded::<ArcPacket>();

        let encoders = decoders
            .iter_mut()
            .scan(&mut info, |info, dec| {
                println!("index {}", dec.0);
                let st = &demuxer.info.streams[*dec.0];
                // TODO: stream selection and mapping
                if let Some(ref codec_id) = st.params.codec_id {
                    if let Some(mut ctx) = EncoderCtx::by_name(&encoder_list, codec_id) {
                        // Derive a default setup from the input codec parameters
                        debug!("Setting up {} encoder", codec_id);
                        ctx.set_params(&st.params).unwrap();
                        // Overrides here
                        let _ = ctx
                            .set_option("timebase", (*st.timebase.numer(), *st.timebase.denom()));
                        ctx.configure().unwrap();
                        let idx = info.add_stream(Stream::from_params(
                            &ctx.get_params().unwrap(),
                            st.timebase.clone(),
                        ));
                        // decoder -> encoder
                        let (send_frame, recv_frame) = ch::unbounded::<ArcFrame>();

                        (dec.1).1 = Some(send_frame);
                        let send_packet = send_packet.clone();
                        let b = thread::Builder::new().name(format!("encoder-{}", codec_id));
                        let th =
                            b.spawn(move || {
                                debug!("Encoding thread");
                                while let Some(frame) = recv_frame.recv() {
                                    debug!("Encoding {:?}", frame);
                                    let _ = ctx.send_frame(&frame).map_err(|e| {
                                      error!("ctx.send_frame: {:?}", e);
                                      e
                                    });

                                    while let Some(mut pkt) = ctx.receive_packet().map_err(|e| {
                                        use codec::error::*;
                                        match e {
                                            Error::MoreDataNeeded => (),
                                            _ => {
                                                error!("flush ctx.receive_packet: {:?}", e);
                                            },
                                        }
                                        e
                                    }).ok() {
                                        pkt.stream_index = idx as isize;
                                        debug!("Encoded {:?}", pkt);

                                        send_packet.send(Arc::new(pkt));
                                    }
                                }

                                ctx.flush().map_err(|e| {
                                  error!("ctx flush: {:?}", e);
                                  e
                                });
                                while let Some(mut pkt) = ctx.receive_packet().map_err(|e| {
                                    use codec::error::*;
                                    match e {
                                        Error::MoreDataNeeded => (),
                                        _ => {
                                            error!("flush ctx.receive_packet: {:?}", e);
                                        },
                                    }
                                    e
                                }).ok() {
                                    pkt.stream_index = idx as isize;

                                    send_packet.send(Arc::new(pkt));
                                }

                            }).unwrap();
                        debug!("Done");
                        Some(th)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();

        (encoders, recv_packet)
    };

    info!("Encoders set {:?}", info);

    let mut sink = Sink::from_path(&opt.output, info);

    let b = thread::Builder::new().name("decode".to_owned());
    let th_src = b.spawn(move || {
        while let Ok(_) = src.decode_one() {}
    }).unwrap();

    let b = thread::Builder::new().name("mux".to_owned());
    let th_mux =
        b.spawn(move || {
            while let Some(pkt) = recv_packet.recv() {
                let _ = sink.write_packet(pkt);
            }
            let _ = sink.write_trailer();
        }).unwrap();

    let _ = th_src.join();
    let _ = th_mux.join();
}
