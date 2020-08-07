extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;
extern crate gstreamer_video as gst_video;
extern crate gdk_pixbuf;
extern crate glib;
extern crate gio;

use gst::prelude::*;

use anyhow::Error;
use derive_more::{Display, Error};

use std::env;
use std::io;

#[derive(Debug, Display, Error)]
#[display(fmt = "Missing element {}", _0)]
struct MissingElement(#[error(not(source))] &'static str);

#[derive(Debug, Display, Error)]
#[display(fmt = "Processing error {}", _0)]
struct ProcessingError(#[error(not(source))] &'static str);


#[derive(Debug, Display, Error)]
#[display(fmt = "Received error from {}: {} (debug: {:?})", src, error, debug)]
struct ErrorMessage {
    src: String,
    error: String,
    debug: Option<String>,
    source: glib::Error,
}

/* Create a new Pipeline */
fn create_pipeline(uri: &String) -> Result<gst::Pipeline, Error> {
    gst::init()?;

    let caps = "video/x-raw,format=RGB,pixel-aspect-ratio=1/1";
    let descr = format!("uridecodebin uri={} ! videoconvert ! videoscale ! timeoverlay ! appsink name=sink caps={}", uri, caps);
    let mut context = gst::ParseContext::new();
    let pipeline =  gst::parse_launch_full(&descr, Some(&mut context), gst::ParseFlags::empty())?;
    Ok(pipeline.dynamic_cast::<gst::Pipeline>().unwrap())
}

fn sample_to_png(sample: &gst::Sample, filename: &String) -> Result<(), Error> {
    println!("Sample: {:#?}", sample);
    let caps = sample.get_caps().unwrap();
    let s = caps.get_structure(0).unwrap();
    let width = s.get_some::<i32>("width")?;
    let height= s.get_some::<i32>("height")?;
    let buffer = sample.get_buffer().unwrap();
    let map = buffer.map_readable()?;

    let bytes = glib::Bytes::from(map.as_slice());
    let pixbuf = gdk_pixbuf::Pixbuf::from_bytes(&bytes,
                                                gdk_pixbuf::Colorspace::Rgb,
                                                false,
                                                8,
                                                width,
                                                height,
                                                width * 3);

    //let _save = pixbuf.savev(filename, "png", &[]);
    //let _save = pixbuf.save_to_bufferv("png", &[]);

    let stdout = io::stderr();
    let iooutput = gio::WriteOutputStream::new(stdout);

    let fut = pixbuf.save_to_streamv(&iooutput, "jpeg", &[], None as Option<&gio::Cancellable>);

    Ok(())
}

fn main() {
    let args: Vec<_> = env::args().collect();
    let pipeline = create_pipeline(&args[1]).unwrap();

    /* get sink */
    let sink = pipeline.get_by_name("sink").unwrap();

    /* set to PAUSED to make the first frame arrive in the sink */
    pipeline.set_state(gst::State::Paused)
        .expect("Unable to set the pipeline to the `Paused` state");

    let (res, _, _ ) =pipeline.get_state(gst::ClockTime::from_seconds(5));
    res.expect("pipeline state query failed");

    let duration = pipeline.query_duration::<gst::ClockTime>().unwrap_or(gst::ClockTime::from_seconds(0));

    let percent = args[3].parse::<u64>().unwrap_or(5);
    let position = duration.nseconds().unwrap() * percent / 100;

    println!("Duration: {}, seek to {}%: {}s", duration, percent, position);

    pipeline
        .seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, gst::ClockTime::from_nseconds(position))
        .expect("Unable to seek in the media");

    let appsink = sink.dynamic_cast::<gst_app::AppSink>().unwrap();
    let sample = appsink.pull_preroll().unwrap();

    match sample_to_png(&sample, &args[2]) {
        Ok(_) => println!("Snapshot saved!"),
        Err(e) => eprintln!("Error! {}", e),
    }
}
