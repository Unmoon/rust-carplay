#![allow(dead_code)]

use crate::driver::{DongleConfig, DongleDriver};
use crate::message::Message;
use futures::executor::block_on;
use gstreamer::glib;
use gstreamer::prelude::{Cast, ElementExt, GstBinExtManual};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use test_log::env_logger;
use tokio::sync::broadcast::{Receiver, Sender, channel};

use crate::readable::ReadableMessage;
use gstreamer::glib::SourceId;
use gstreamer_app::prelude::ObjectExt;
use log::error;

mod commands;
mod driver;
mod message;
mod messagetypes;
mod readable;
mod sendable;

async fn setup_dongle(tx: Sender<Message>) {
    let mut dongle = DongleDriver::new();
    let config = DongleConfig {
        android_work_mode: Some(false),
        box_name: String::from("test"),
        night_mode: true,
        width: 1920,
        height: 1080,
        fps: 60,
        ..Default::default()
    };
    block_on(dongle.initialize()).unwrap();
    block_on(dongle.start(config)).unwrap();
    block_on(dongle.run(tx));
}

#[tokio::main]
pub async fn main() {
    env_logger::init();

    let (tx, _) = channel(64);

    tokio::spawn(streamer(tx.clone()));

    setup_dongle(tx.clone()).await;
}

async fn streamer(tx: Sender<Message>) {
    if let Err(err) = gstreamer::init() {
        error!("Failed to initialize gstreamer: {err}");
        return;
    }

    let appsrc = gstreamer_app::AppSrc::builder()
        .name("video_source")
        .build();
    let parser = gstreamer::ElementFactory::make("h264parse")
        .build()
        .unwrap();
    let decoder = gstreamer::ElementFactory::make("avdec_h264")
        .build()
        .unwrap();
    let video_convert = gstreamer::ElementFactory::make("videoconvert")
        .name("video_convert")
        .build()
        .unwrap();
    let video_sink = gstreamer::ElementFactory::make("autovideosink")
        .property("sync", false)
        .build()
        .unwrap();
    let appsink = gstreamer_app::AppSink::builder().name("app_sink").build();

    appsrc.set_property("stream-type", gstreamer_app::AppStreamType::Stream);
    appsrc.set_property("is-live", true);
    let pipeline = gstreamer::Pipeline::with_name("test-pipeline");

    pipeline
        .add_many([
            appsrc.upcast_ref(),
            &parser,
            &decoder,
            &video_convert,
            &video_sink,
            appsink.upcast_ref(),
        ])
        .unwrap();

    gstreamer::Element::link_many([
        appsrc.upcast_ref(),
        &parser,
        &decoder,
        &video_convert,
        &video_sink,
    ])
    .unwrap();

    let data: Arc<Mutex<CustomData>> =
        Arc::new(Mutex::new(CustomData::new(&appsrc, &appsink, tx.clone())));

    let data_weak = Arc::downgrade(&data);
    let data_weak2 = Arc::downgrade(&data);

    appsrc.set_callbacks(
        gstreamer_app::AppSrcCallbacks::builder()
            .need_data(move |_appsrc, _| {
                let Some(data) = data_weak.upgrade() else {
                    return;
                };
                let mut d = data.lock().unwrap();
                let mut rx = d.tx.subscribe();
                let video_sink = video_sink.clone();

                if d.source_id.is_none() {
                    let data_weak = Arc::downgrade(&data);
                    d.source_id = Some(glib::source::idle_add(move || {
                        let Some(data) = data_weak.upgrade() else {
                            return glib::ControlFlow::Break;
                        };

                        let (appsrc, buffer) = {
                            let data = data.lock().unwrap();
                            let mut buffer;

                            loop {
                                match rx.try_recv() {
                                    Ok(Message::ReadVideoData(msg)) => {
                                        buffer = gstreamer::Buffer::from_mut_slice(msg.get_data());
                                        buffer
                                            .get_mut()
                                            .unwrap()
                                            .set_pts(gstreamer::ClockTime::NONE);
                                        break;
                                    }
                                    _ => {
                                        sleep(Duration::from_secs_f32(0.01));
                                    }
                                }
                            }
                            (data.appsrc.clone(), buffer)
                        };

                        let ok = appsrc.push_buffer(buffer).is_ok();
                        video_sink.set_state(gstreamer::State::Playing).unwrap();
                        glib::ControlFlow::from(ok)
                    }));
                }
            })
            .enough_data(move |_| {
                let Some(data) = data_weak2.upgrade() else {
                    return;
                };

                let mut data = data.lock().unwrap();
                if let Some(source) = data.source_id.take() {
                    source.remove();
                }
            })
            .build(),
    );

    let main_loop = glib::MainLoop::new(None, false);
    pipeline
        .set_state(gstreamer::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state.");

    main_loop.run();
}

#[derive(Debug)]
struct CustomData {
    source_id: Option<SourceId>,

    appsrc: gstreamer_app::AppSrc,
    appsink: gstreamer_app::AppSink,
    tx: Sender<Message>,
    rx: Receiver<Message>,
}

impl CustomData {
    fn new(
        appsrc: &gstreamer_app::AppSrc,
        appsink: &gstreamer_app::AppSink,
        tx: Sender<Message>,
    ) -> CustomData {
        CustomData {
            source_id: None,
            appsrc: appsrc.clone(),
            appsink: appsink.clone(),
            tx: tx.clone(),
            rx: tx.subscribe(),
        }
    }
}
