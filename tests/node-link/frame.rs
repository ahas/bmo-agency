//! A frame of the link reads back as it was written.

use node_link::frame::*;

#[test]
fn a_frame_reads_back_as_written() {
  let frame = Frame::Append {
    pull: 2,
    path: "a/b.jsonl".into(),
    offset: 9,
    data: encode(b"{\"x\":1}\n"),
  };
  let back = Frame::read(&frame.line()).expect("reads");

  assert_eq!(back, frame);
}
