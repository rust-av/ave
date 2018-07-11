use std::fs::File;
use std::path::Path;

use codec::common::CodecList;
use data::frame::ArcFrame;

use codec::decoder::Codecs as Decoders;
use codec::decoder::Context as DecoderCtx;

use format::buffer::AccReader;
use format::demuxer::Context as DemuxerCtx;
use format::demuxer::Event;
use opus::decoder::OPUS_DESCR as OPUS_DEC;
use vorbis::decoder::VORBIS_DESCR as VORBIS_DEC;
use vpx::decoder::VP9_DESCR as VP9_DEC;

use matroska::demuxer::MkvDemuxer;
use std::collections::HashMap;

use channel::Sender;

/// The source binds a single Demuxer
/// to as many Decoders as the Streams
pub struct Source {
    pub decoders: HashMap<usize, (DecoderCtx, Option<Sender<ArcFrame>>)>,
    pub demuxer: DemuxerCtx,
}

impl Source {
    /// Creates a source from a path
    // TODO:
    // - use multiple demuxers
    // - make the codec list allocation external
    pub fn from_path<P: AsRef<Path>>(path: P) -> Self {
        let decoder_list = Decoders::from_list(&[VP9_DEC, OPUS_DEC, VORBIS_DEC]);

        let r = File::open(path).unwrap();
        let ar = AccReader::with_capacity(4 * 1024, r);
        let mut demuxer = DemuxerCtx::new(Box::new(MkvDemuxer::new()), Box::new(ar));
        demuxer
            .read_headers()
            .expect("Cannot parse the format headers");

        let mut decoders: HashMap<usize, (DecoderCtx, Option<Sender<ArcFrame>>)> =
            HashMap::with_capacity(demuxer.info.streams.len());

        for st in &demuxer.info.streams {
            if let Some(ref codec_id) = st.params.codec_id {
                if let Some(mut ctx) = DecoderCtx::by_name(&decoder_list, codec_id) {
                    if let Some(ref extradata) = st.params.extradata {
                        ctx.set_extradata(extradata);
                    }
                    ctx.configure().expect("Codec configure failed");
                    decoders.insert(st.index, (ctx, None));
                    info!(
                        "Registering {} for stream {} (id {})",
                        codec_id, st.index, st.id
                    );
                }
            }
        }

        Source { decoders, demuxer }
    }

    pub fn decode_one(&mut self) -> Result<(), String> {
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
                            dec.0.send_packet(&pkt).unwrap();
                            if let Some(frame) = dec.0.receive_frame().ok() {
                                dec.1.as_mut().unwrap().send(frame);
                            }
                            Ok(())
                        } else {
                            debug!("Skipping packet at index {}", pkt.stream_index);
                            Ok(())
                        }
                    } else {
                        warn!("Spurious packet");
                        Ok(())
                    }
                }
                Event::Eof => Err("EOF".to_owned()),
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
