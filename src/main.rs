#![allow(dead_code)]

use crate::driver::{DongleConfig, DongleDriver};
use crate::message::Message;
use futures::executor::block_on;
use gstgtk4::PaintableSink;
use gstreamer::prelude::{Cast, ElementExt, GstBinExtManual};
use gstreamer::{ElementFactory, glib};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;
use test_log::env_logger;
use tokio::sync::broadcast::{Receiver, Sender, channel};

use crate::readable::ReadableMessage;
use crate::sendable::{SendTouch, SendableMessage, TouchAction};
use gstreamer::glib::SourceId;
use gstreamer_app::prelude::ObjectExt;
use gtk::gdk::Paintable;
use gtk::prelude::{
    ApplicationExt, ApplicationExtManual, BoxExt, GestureSingleExt, GtkWindowExt, WidgetExt,
};
use gtk::{Application, ApplicationWindow, Orientation};
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
    let mut dongle = DongleDriver::new(dongle_tx, dongle_rx);
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
    let (dongle_tx, dongle_rx) = mpsc::channel(32);

    tokio::spawn(streamer(tx.clone(), dongle_tx.clone()));

    setup_dongle(tx.clone(), dongle_tx, dongle_rx).await;
}

async fn streamer(tx: Sender<Message>, dongle_tx: mpsc::Sender<Box<dyn SendableMessage + Send>>) {
    gstreamer::init().unwrap();
    gtk::init().unwrap();
    gstgtk4::plugin_register_static().expect("Failed to register gstgtk4 plugin");

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
    let video_queue = gstreamer::ElementFactory::make("queue")
        .name("video_queue")
        .build()
        .unwrap();
    let sink = ElementFactory::make("gtk4paintablesink").build().unwrap();
    let gtk4paintablesink = sink
        .dynamic_cast::<PaintableSink>()
        .expect("Sink element is expected to be a Gtk4PaintableSink!");

    let paintable = gtk4paintablesink.property::<Paintable>("paintable");
    let image = gtk::Picture::for_paintable(&paintable);
    let video_box = gtk::Box::new(Orientation::Vertical, 0);
    video_box.append(&image);

    appsrc.set_property("stream-type", gstreamer_app::AppStreamType::Stream);
    appsrc.set_property("is-live", true);
    let pipeline = gstreamer::Pipeline::with_name("test-pipeline");

    pipeline
        .add_many([
            appsrc.upcast_ref(),
            &parser,
            &decoder,
            &video_convert,
            &video_queue,
            (&gtk4paintablesink).as_ref(),
        ])
        .unwrap();

    gstreamer::Element::link_many([
        appsrc.upcast_ref(),
        &parser,
        &decoder,
        &video_convert,
        &video_queue,
        (&gtk4paintablesink).as_ref(),
    ])
    .unwrap();

    let data: Arc<Mutex<CustomData>> = Arc::new(Mutex::new(CustomData::new(&appsrc, tx.clone())));

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

    // Connect mouse events
    let mouse_data: Arc<Mutex<MouseData>> = Arc::new(Mutex::new(MouseData {
        mouse_down: false,
        mouse_x: 0.0,
        mouse_y: 0.0,
    }));
    let motion_controller = gtk::EventControllerMotion::new();
    let dongle_tx_clone = dongle_tx.clone();
    let data_weak = Arc::downgrade(&mouse_data);
    motion_controller.connect_motion(move |_controller, x, y| {
        println!("in connect_motion");
        let Some(data) = data_weak.upgrade() else {
            return;
        };
        let mut d = data.lock().unwrap();
        if d.mouse_down {
            d.mouse_x = (x / 1920.0) as f32;
            d.mouse_y = (y / 1080.0) as f32;
            let message = SendTouch::new(d.mouse_x, d.mouse_y, TouchAction::Move);
            dongle_tx_clone.try_send(Box::new(message)).unwrap();
            println!("Mouse moved to ({}, {})", x, y);
        }
    });
    let dongle_tx_clone = dongle_tx.clone();
    let data_weak = Arc::downgrade(&mouse_data);
    motion_controller.connect_leave(move |_controller| {
        let Some(data) = data_weak.upgrade() else {
            return;
        };
        let mut d = data.lock().unwrap();
        d.mouse_down = false;
        let message = SendTouch::new(d.mouse_x, d.mouse_y, TouchAction::Up);
        dongle_tx_clone.try_send(Box::new(message)).unwrap();
        println!("Mouse left");
    });
    video_box.add_controller(motion_controller);
    let click_controller = gtk::GestureClick::new();
    let dongle_tx_clone = dongle_tx.clone();
    let data_weak = Arc::downgrade(&mouse_data);
    click_controller.connect_pressed(move |gesture, _n_press, x, y| {
        println!("in connect_pressed");
        let Some(data) = data_weak.upgrade() else {
            return;
        };
        let mut d = data.lock().unwrap();
        d.mouse_down = false;
        d.mouse_x = (x / 1920.0) as f32;
        d.mouse_y = (y / 1080.0) as f32;
        let message = SendTouch::new(d.mouse_x, d.mouse_y, TouchAction::Down);
        dongle_tx_clone.try_send(Box::new(message)).unwrap();
        println!(
            "Mouse button {} pressed at ({}, {})",
            gesture.current_button(),
            x,
            y
        );
    });
    let dongle_tx_clone = dongle_tx.clone();
    let data_weak = Arc::downgrade(&mouse_data);
    click_controller.connect_released(move |gesture, _n_press, x, y| {
        println!("in connect_released");
        let Some(data) = data_weak.upgrade() else {
            return;
        };
        let mut d = data.lock().unwrap();
        d.mouse_down = false;
        d.mouse_x = (x / 1920.0) as f32;
        d.mouse_y = (y / 1080.0) as f32;
        let message = SendTouch::new(d.mouse_x, d.mouse_y, TouchAction::Up);
        dongle_tx_clone.try_send(Box::new(message)).unwrap();
        println!(
            "Mouse button {} released at ({}, {})",
            gesture.current_button(),
            x,
            y
        );
    });
    video_box.add_controller(click_controller);

    pipeline
        .set_state(gstreamer::State::Playing)
        .expect("Unable to set the pipeline to the `Playing` state.");

    let app = Application::builder()
        .application_id("com.example.Gtk4Gstreamer")
        .build();

    app.connect_activate(move |app| {
        let window = ApplicationWindow::builder()
            .application(app)
            .title("GTK4 GStreamer Example")
            .default_width(1920)
            .default_height(1080)
            .build();

        window.set_child(Some(&video_box));
        window.show();
    });
    app.run();
}

struct MouseData {
    mouse_down: bool,
    mouse_x: f32,
    mouse_y: f32,
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
