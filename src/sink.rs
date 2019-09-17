use std::fs::File;
use std::path::Path;
use std::sync::Arc;

use data::packet::Packet;

use format;
use format::common::GlobalInfo;
use format::muxer::Context as MuxerCtx;

use ivf::muxer::IvfMuxer;
use matroska::muxer::MkvMuxer;

pub struct Sink {
    muxer: MuxerCtx,
}

impl Sink {
    pub fn from_path(path: &Path, info: GlobalInfo) -> Self {
        let output = File::create(path).unwrap();

        let mut muxer = match path.to_owned().extension().unwrap().to_str() {
            Some("ivf") => MuxerCtx::new(Box::new(IvfMuxer::new()), Box::new(output)),
            Some("webm") => MuxerCtx::new(Box::new(MkvMuxer::webm()), Box::new(output)),
            _ => MuxerCtx::new(Box::new(MkvMuxer::matroska()), Box::new(output)),
        };

        muxer.set_global_info(info).unwrap();
        muxer.configure().unwrap();
        muxer.write_header().unwrap();

        Sink { muxer }
    }

    pub fn write_packet(&mut self, packet: Arc<Packet>) -> format::error::Result<usize> {
        self.muxer.write_packet(packet)
    }

    pub fn write_trailer(&mut self) -> format::error::Result<usize> {
        self.muxer.write_trailer()
    }
}
