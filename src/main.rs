#![allow(dead_code)]

use crate::driver::read_loop;
use crate::driver::send_loop;
use crate::driver::DongleConfig;
use crate::driver::DongleDriver;
use crate::message::Message;
use futures::executor::block_on;
use gstgtk4::PaintableSink;
use gstreamer::prelude::ElementExt;
use gstreamer::prelude::GstBinExtManual;
use gstreamer::prelude::{Cast, GstObjectExt};
use gstreamer::ElementFactory;
use gstreamer::{glib, MessageView};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::RwLock;
use std::thread::sleep;
use std::time::Duration;
use test_log::env_logger;
use tokio::sync::broadcast::channel;
use tokio::sync::broadcast::Receiver;
use tokio::sync::broadcast::Sender;

use crate::readable::ReadableMessage;
use crate::sendable::SendTouch;
use crate::sendable::SendableMessage;
use crate::sendable::TouchAction;
use gstreamer::glib::SourceId;
use gstreamer_audio::AudioInfo;
use gtk::prelude::ApplicationExt;
use gtk::prelude::ApplicationExtManual;
use gtk::prelude::BoxExt;
use gtk::prelude::GestureExt;
use gtk::prelude::GtkWindowExt;
use gtk::prelude::WidgetExt;
use gtk::Application;
use gtk::ApplicationWindow;
use gtk::Orientation;
use log::error;
use tokio::sync::mpsc;

mod commands;
mod driver;
mod message;
mod messagetypes;
mod readable;
mod sendable;

async fn setup_dongle(
    tx: Sender<Message>,
    dongle_tx: mpsc::Sender<Box<dyn SendableMessage + Send>>,
    dongle_rx: mpsc::Receiver<Box<dyn SendableMessage + Send>>,
) {
    let mut dongle = DongleDriver::new();
    let config = DongleConfig {
        android_work_mode: Some(false),
        box_name: String::from("test"),
        night_mode: true,
        width: 1920,
        height: 1080,
        fps: 60,
        media_delay: 100,
        ..Default::default()
    };
    block_on(dongle.initialize()).unwrap();
    block_on(dongle.start(config, dongle_tx)).unwrap();
    let in_ep = dongle.in_ep.unwrap().clone();
    let out_ep = dongle.out_ep.unwrap().clone();
    let interface = dongle.interface.unwrap();
    tokio::spawn(read_loop(in_ep, interface.clone(), tx.clone()));
    let rx_mutex = Arc::new(tokio::sync::Mutex::new(dongle_rx));
    tokio::spawn(send_loop(out_ep, interface.clone(), rx_mutex.clone()));
}

pub fn main() {
    env_logger::init();
    gstreamer::init().unwrap();
    gtk::init().unwrap();
    gstgtk4::plugin_register_static().expect("Failed to register gstgtk4 plugin");

    let (tx, _) = channel(64);
    let (dongle_tx, dongle_rx) = mpsc::channel(64);
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let d = rt.spawn(setup_dongle(tx.clone(), dongle_tx.clone(), dongle_rx));

    let a = rt.spawn(audio(tx.clone()));
    video_streamer_and_gui(tx.clone(), dongle_tx.clone());
    match block_on(d) {
        Ok(_) => {}
        Err(e) => {
            error!("Dongle ended in error: {}", e);
        }
    }

    match block_on(a) {
        Ok(_) => {}
        Err(e) => {
            error!("Audio ended in error: {}", e);
        }
    }
}

async fn audio(tx: Sender<Message>) {
    let appsrc = gstreamer_app::AppSrc::builder()
        .name("audio_source")
        .stream_type(gstreamer_app::AppStreamType::Stream)
        .is_live(true)
        .block(true)
        .build();

    let audio_queue = ElementFactory::make("queue")
        .name("audio_queue")
        .build()
        .unwrap();
    let audio_convert = ElementFactory::make("audioconvert")
        .name("audio_convert")
        .build()
        .unwrap();
    let audio_resample = ElementFactory::make("audioresample")
        .name("audio_resample")
        .build()
        .unwrap();
    let audio_sink = ElementFactory::make("autoaudiosink")
        .name("audio_sink")
        .build()
        .unwrap();

    let pipeline = gstreamer::Pipeline::with_name("audio-pipeline");

    pipeline
        .add_many([
            appsrc.upcast_ref(),
            &audio_queue,
            &audio_convert,
            &audio_resample,
            &audio_sink,
        ])
        .unwrap();

    gstreamer::Element::link_many([
        appsrc.upcast_ref(),
        &audio_queue,
        &audio_convert,
        &audio_resample,
        &audio_sink,
    ])
    .unwrap();

    let data: Arc<Mutex<CustomData>> = Arc::new(Mutex::new(CustomData::new(&appsrc, tx.clone())));
    appsrc.set_callbacks(
        gstreamer_app::AppSrcCallbacks::builder()
            .need_data(glib::clone!(
                #[weak]
                data,
                move |_appsrc, _| {
                    let mut d = data.lock().unwrap();
                    if d.source_id.is_some() {
                        return;
                    }

                    d.source_id = Some(glib::source::idle_add(glib::clone!(
                        #[weak]
                        data,
                        #[upgrade_or]
                        glib::ControlFlow::Break,
                        move || {
                            let mut data = data.lock().unwrap();
                            match data.rx.try_recv() {
                                Ok(Message::ReadAudioData(msg)) => {
                                    if msg.data.is_some() {
                                        let audiodata = msg.data.as_ref().unwrap();
                                        let mut buffer =
                                            gstreamer::Buffer::with_size(audiodata.len() * 2)
                                                .expect("Failed to create buffer");
                                        {
                                            let buffer_mut = buffer.make_mut();
                                            let mut buffer_map = buffer_mut
                                                .map_writable()
                                                .expect("Failed to map buffer");

                                            // Copy the i16 data into the buffer (i16 is 2 bytes)
                                            let buffer_slice = buffer_map.as_mut_slice();
                                            for (chunk, &sample) in buffer_slice
                                                .chunks_exact_mut(2)
                                                .zip(audiodata.iter())
                                            {
                                                let bytes = sample.to_ne_bytes();
                                                chunk.copy_from_slice(&bytes);
                                            }
                                        }
                                        let format = msg.get_audio_format().unwrap();
                                        let info = AudioInfo::builder(
                                            gstreamer_audio::AudioFormat::S16le,
                                            format.sample_rate,
                                            format.channels as u32,
                                        )
                                        .build()
                                        .unwrap();
                                        let audio_caps = info.to_caps().unwrap();
                                        let sample = gstreamer::Sample::builder()
                                            .buffer(&buffer)
                                            .caps(&audio_caps)
                                            .build();
                                        data.appsrc.clone().push_sample(&sample).unwrap();
                                    }
                                }
                                _ => {
                                    sleep(Duration::from_secs_f32(0.01));
                                }
                            };

                            glib::ControlFlow::Continue
                        }
                    )))
                }
            ))
            .build(),
    );

    let main_loop = glib::MainLoop::new(None, false);
    let main_loop_clone = main_loop.clone();
    let bus = pipeline.bus().unwrap();
    #[allow(clippy::single_match)]
    bus.connect_message(Some("error"), move |_, msg| match msg.view() {
        MessageView::Error(err) => {
            let main_loop = &main_loop_clone;
            eprintln!(
                "Error received from element {:?}: {}",
                err.src().map(|s| s.path_string()),
                err.error()
            );
            eprintln!("Debugging information: {:?}", err.debug());
            main_loop.quit();
        }
        _ => unreachable!(),
    });
    bus.add_signal_watch();

    pipeline
        .set_state(gstreamer::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state.");

    main_loop.run();

    pipeline
        .set_state(gstreamer::State::Null)
        .expect("Unable to set the pipeline to the `Null` state.");

    bus.remove_signal_watch();
}

fn video_streamer_and_gui(
    tx: Sender<Message>,
    dongle_tx: mpsc::Sender<Box<dyn SendableMessage + Send>>,
) {
    let appsrc = gstreamer_app::AppSrc::builder()
        .name("video_source")
        .stream_type(gstreamer_app::AppStreamType::Stream)
        .is_live(true)
        .build();

    let parser = ElementFactory::make("h264parse").build().unwrap();
    let decoder = ElementFactory::make("avdec_h264").build().unwrap();
    let video_convert = ElementFactory::make("videoconvert")
        .name("video_convert")
        .build()
        .unwrap();
    let video_queue = ElementFactory::make("queue")
        .name("video_queue")
        .build()
        .unwrap();
    let video_sink = ElementFactory::make("gtk4paintablesink")
        .build()
        .unwrap()
        .dynamic_cast::<PaintableSink>()
        .expect("Sink element is expected to be a Gtk4PaintableSink!");

    let gst_widget = gstgtk4::RenderWidget::new(video_sink.as_ref());

    let pipeline = gstreamer::Pipeline::with_name("video-pipeline");

    pipeline
        .add_many([
            appsrc.upcast_ref(),
            &parser,
            &decoder,
            &video_convert,
            &video_queue,
            (&video_sink).as_ref(),
        ])
        .unwrap();

    gstreamer::Element::link_many([
        appsrc.upcast_ref(),
        &video_queue,
        &parser,
        &decoder,
        &video_convert,
        (&video_sink).as_ref(),
    ])
    .unwrap();

    let data: Arc<Mutex<CustomData>> = Arc::new(Mutex::new(CustomData::new(&appsrc, tx.clone())));
    appsrc.set_callbacks(
        gstreamer_app::AppSrcCallbacks::builder()
            .need_data(glib::clone!(
                #[weak]
                data,
                move |_appsrc, _| {
                    let mut d = data.lock().unwrap();
                    if d.source_id.is_some() {
                        return;
                    }

                    d.source_id = Some(glib::source::idle_add(glib::clone!(
                        #[weak]
                        data,
                        #[upgrade_or]
                        glib::ControlFlow::Break,
                        move || {
                            let mut data = data.lock().unwrap();
                            match data.rx.try_recv() {
                                Ok(Message::ReadVideoData(msg)) => {
                                    data.appsrc
                                        .clone()
                                        .push_buffer(gstreamer::Buffer::from_mut_slice(
                                            msg.get_data(),
                                        ))
                                        .unwrap();
                                }
                                _ => {
                                    sleep(Duration::from_secs_f32(0.01));
                                }
                            };

                            glib::ControlFlow::Continue
                        }
                    )))
                }
            ))
            .build(),
    );

    let video_box = gtk::Box::new(Orientation::Vertical, 0);
    video_box.append(&gst_widget);

    let drag_controller = gtk::EventControllerMotion::new();
    let mouse_down_main = Arc::new(RwLock::new(false));

    let dongle_tx_clone = dongle_tx.clone();
    let mouse_down = mouse_down_main.clone();
    drag_controller.connect_motion(move |_controller, x, y| {
        if *mouse_down.read().unwrap() {
            let x = (x / 1920.0) as f32;
            let y = (y / 1080.0) as f32;
            let message = SendTouch::new(x, y, TouchAction::Move);
            let dongle_tx_clone = dongle_tx_clone.clone();
            match block_on(dongle_tx_clone.send(Box::new(message))) {
                Ok(_) => {}
                Err(e) => {
                    error!("Error sending TouchAction::Down {}", e)
                }
            }
        }
    });
    video_box.add_controller(drag_controller);

    let click_controller = gtk::GestureClick::new();
    let dongle_tx_clone = dongle_tx.clone();
    let mouse_down = mouse_down_main.clone();
    click_controller.connect_pressed(move |_gesture, _n_press, x, y| {
        let mut m = mouse_down.write().unwrap();
        *m = true;
        drop(m);
        let x = (x / 1920.0) as f32;
        let y = (y / 1080.0) as f32;
        let message = SendTouch::new(x, y, TouchAction::Down);

        let dongle_tx_clone = dongle_tx_clone.clone();
        match block_on(dongle_tx_clone.send(Box::new(message))) {
            Ok(_) => {}
            Err(e) => {
                error!("Error sending TouchAction::Down {}", e)
            }
        }
    });
    let dongle_tx_clone = dongle_tx.clone();
    let mouse_down = mouse_down_main.clone();
    click_controller.connect_released(move |_gesture, _n_press, x, y| {
        let mut m = mouse_down.write().unwrap();
        *m = false;
        drop(m);
        let x = (x / 1920.0) as f32;
        let y = (y / 1080.0) as f32;
        let dongle_tx_clone = dongle_tx_clone.clone();
        let message = SendTouch::new(x, y, TouchAction::Up);
        match block_on(dongle_tx_clone.send(Box::new(message))) {
            Ok(_) => {}
            Err(e) => {
                error!("Error sending TouchAction::Up {}", e)
            }
        }
    });
    let dongle_tx_clone = dongle_tx.clone();
    let mouse_down = mouse_down_main.clone();
    click_controller.connect_end(move |_gesture, _seq| {
        let (x, y) = match _gesture.point(_seq) {
            None => return,
            Some((x, y)) => (x, y),
        };
        let mut m = mouse_down.write().unwrap();
        *m = false;
        drop(m);
        let x = (x / 1920.0) as f32;
        let y = (y / 1080.0) as f32;
        let dongle_tx_clone = dongle_tx_clone.clone();
        let message = SendTouch::new(x, y, TouchAction::Up);
        match block_on(dongle_tx_clone.send(Box::new(message))) {
            Ok(_) => {}
            Err(e) => {
                error!("Error sending TouchAction::Up {}", e)
            }
        }
    });
    video_box.add_controller(click_controller);

    let bus = pipeline.bus().unwrap();
    #[allow(clippy::single_match)]
    bus.connect_message(Some("error"), move |_, msg| match msg.view() {
        MessageView::Error(err) => {
            eprintln!(
                "Error received from element {:?}: {}",
                err.src().map(|s| s.path_string()),
                err.error()
            );
            eprintln!("Debugging information: {:?}", err.debug());
        }
        _ => unreachable!(),
    });
    bus.add_signal_watch();

    pipeline
        .set_state(gstreamer::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state.");

    let app = Application::builder()
        .application_id("com.unmoon.rustcarplay")
        .build();

    app.connect_activate(move |app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("Rust CarPlay")
            .fullscreened(true)
            .build();

        window.set_child(Some(&video_box));
        window.show();
    });
    app.run();
}

#[derive(Debug)]
struct CustomData {
    source_id: Option<SourceId>,

    appsrc: gstreamer_app::AppSrc,
    tx: Sender<Message>,
    rx: Receiver<Message>,
}

impl CustomData {
    fn new(appsrc: &gstreamer_app::AppSrc, tx: Sender<Message>) -> CustomData {
        CustomData {
            source_id: None,
            appsrc: appsrc.clone(),
            tx: tx.clone(),
            rx: tx.subscribe(),
        }
    }
}
