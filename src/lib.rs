mod stream_frame;

pub use stream_frame::FrameWriter;

pub mod prelude {
    pub use super::FrameWriter;
}
