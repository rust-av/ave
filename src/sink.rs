use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use data::packet::Packet;

use format;
use format::common::GlobalInfo;
use format::muxer::Context as MuxerCtx;

use matroska::muxer::MkvMuxer;

pub struct Sink {
    muxer: MuxerCtx,
}

impl Sink {
    pub fn from_path<P: AsRef<Path>>(path: P, info: GlobalInfo) -> Self {
        let mux = Box::new(MkvMuxer::webm());
        let output = File::create(path).unwrap();
        let mut muxer = MuxerCtx::new(mux, Box::new(output));
        muxer.set_global_info(info).unwrap();
        muxer.write_header().unwrap();

        Sink { muxer }
    }

    fn write_packet(&mut self, packet: Arc<Packet>) -> format::error::Result<usize> {
        self.muxer.write_packet(packet)
    }

    fn write_trailer(&mut self) -> format::error::Result<usize> {
        self.muxer.write_trailer()
    }
}
