use crate::commands::CommandMapping::*;
use crate::message::{Message, MessageHeader};
use crate::messagetypes::MessageType::VideoData;
use crate::sendable::SendableMessage;
use log::error;
use nusb;
use nusb::transfer::{Direction, RequestBuffer};
use nusb::{Device, Interface};
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use thiserror::Error;
use tokio::sync::broadcast::Sender;
use tokio::sync::mpsc;
use tokio::time;

const HEADER_DATA_LENGTH: usize = 16;

#[derive(Debug, Error)]
pub enum DriverError {
    #[error("USB error: {0}")]
    UsbError(#[from] nusb::Error),
    #[error("USB error: {0}")]
    UsbTransferError(#[from] nusb::transfer::TransferError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandDriveType {
    Lhd = 0,
    Rhd = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PhoneType {
    AndroidMirror = 1,
    CarPlay = 3,
    IphoneMirror = 4,
    AndroidAuto = 5,
    HiCar = 6,
}

#[derive(Debug, Clone)]
pub struct PhoneTypeConfig {
    pub frame_interval: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct DongleConfig {
    pub android_work_mode: Option<bool>,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub dpi: u32,
    pub format: u32,
    pub i_box_version: u32,
    pub packet_max: u32,
    pub phone_work_mode: u32,
    pub night_mode: bool,
    pub box_name: String,
    pub hand: HandDriveType,
    pub media_delay: u32,
    pub audio_transfer_mode: bool,
    pub wifi_type: WifiType,
    pub mic_type: MicType,
    pub phone_config: HashMap<PhoneType, PhoneTypeConfig>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WifiType {
    Ghz2_4,
    Ghz5,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicType {
    Box,
    Os,
}

impl Default for DongleConfig {
    fn default() -> Self {
        let mut phone_config = HashMap::new();
        phone_config.insert(
            PhoneType::CarPlay,
            PhoneTypeConfig {
                frame_interval: Some(5000),
            },
        );
        phone_config.insert(
            PhoneType::AndroidAuto,
            PhoneTypeConfig {
                frame_interval: None,
            },
        );

        Self {
            android_work_mode: None,
            width: 800,
            height: 640,
            fps: 20,
            dpi: 160,
            format: 5,
            i_box_version: 2,
            phone_work_mode: 2,
            packet_max: 49152,
            box_name: "nodePlay".to_string(),
            night_mode: false,
            hand: HandDriveType::Lhd,
            media_delay: 300,
            audio_transfer_mode: false,
            wifi_type: WifiType::Ghz5,
            mic_type: MicType::Os,
            phone_config,
        }
    }
}

#[derive(Debug)]
pub struct KnownDevice {
    pub vendor_id: u16,
    pub product_id: u16,
}

pub const KNOWN_DEVICES: [KnownDevice; 2] = [
    KnownDevice {
        vendor_id: 0x1314,
        product_id: 0x1520,
    },
    KnownDevice {
        vendor_id: 0x1314,
        product_id: 0x1521,
    },
];

pub struct DongleDriver {
    device: Option<Device>,
    in_ep: Option<u8>,
    out_ep: Option<u8>,
    error_count: Arc<Mutex<u32>>,
    max_error_count: u32,
    heartbeat_handle: Option<tokio::task::JoinHandle<()>>,
    message_tx: mpsc::Sender<Box<dyn SendableMessage + Send>>,
    message_rx: mpsc::Receiver<Box<dyn SendableMessage + Send>>,
    interface: Option<Interface>,
}

impl DongleDriver {
    pub fn new(
        tx: mpsc::Sender<Box<dyn SendableMessage + Send>>,
        rx: mpsc::Receiver<Box<dyn SendableMessage + Send>>,
    ) -> Self {
        Self {
            device: None,
            interface: None,
            in_ep: None,
            out_ep: None,
            error_count: Arc::new(Mutex::new(0)),
            max_error_count: 5,
            heartbeat_handle: None,
            message_tx: tx,
            message_rx: rx,
        }
    }

    async fn reset_usb(&mut self) {
        let mut device_info = nusb::list_devices()
            .unwrap()
            .find(|dev| dev.vendor_id() == 0x1314 && dev.product_id() == 0x1521);
        loop {
            if device_info.is_some() {
                break;
            }
            device_info = nusb::list_devices()
                .unwrap()
                .find(|dev| dev.vendor_id() == 0x1314 && dev.product_id() == 0x1521);
        }
        let device = device_info
            .expect("Here we should have it")
            .open()
            .expect("Not found after reset");
        device.reset().expect("Failed to reset");
    }

    pub async fn initialize(&mut self) -> Result<(), DriverError> {
        // self.reset_usb().await;
        let mut device_info = nusb::list_devices()?
            .find(|dev| dev.vendor_id() == 0x1314 && dev.product_id() == 0x1521);
        loop {
            if device_info.is_some() {
                break;
            }
            device_info = nusb::list_devices()?
                .find(|dev| dev.vendor_id() == 0x1314 && dev.product_id() == 0x1521);
        }
        let device = device_info
            .expect("Not found???")
            .open()
            .expect("Not found after reset");
        device.set_configuration(1)?;
        let config = device.active_configuration().unwrap();
        let interface = config.interfaces().next().unwrap();

        let alt_settings = interface.alt_settings().next().unwrap();
        let in_endpoint = alt_settings
            .endpoints()
            .find(|e| e.direction() == Direction::In)
            .unwrap();
        let out_endpoint = alt_settings
            .endpoints()
            .find(|e| e.direction() == Direction::Out)
            .unwrap();

        self.interface = Some(device.claim_interface(interface.interface_number())?);
        self.device = Some(device.clone());
        self.in_ep = Some(in_endpoint.address());
        self.out_ep = Some(out_endpoint.address());

        Ok(())
    }

    pub async fn send(&self, message: Box<dyn SendableMessage>) -> Result<(), DriverError> {
        // info!("Sending message {:?}", message.message_type());
        let payload = message.serialize();

        let out_ep = self.out_ep.as_ref().unwrap();
        let interface = self.interface.as_ref().unwrap();

        match interface.bulk_out(*out_ep, payload).await.into_result() {
            Ok(_) => Ok(()),
            Err(e) => Err(DriverError::UsbTransferError(e)),
        }
    }

    async fn read_loop(&self, message_tx: Sender<Message>) {
        let in_ep = self.in_ep.as_ref().unwrap().clone();
        let interface = self.interface.as_ref().unwrap().clone();

        tokio::spawn(async move {
            let tx = message_tx.clone();
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .open("foo.mp4")
                .unwrap();
            loop {
                match interface
                    .bulk_in(in_ep, RequestBuffer::new(HEADER_DATA_LENGTH))
                    .await
                    .into_result()
                {
                    Ok(header_data) => {
                        let header = match MessageHeader::from_bytes(&header_data) {
                            Ok(h) => h,
                            Err(e) => {
                                error!("Error parsing header: {}", e);
                                continue;
                            }
                        };
                        // info!("Received message {:?}", header);

                        let extra_data = if header.length > 0 {
                            match interface
                                .bulk_in(in_ep, RequestBuffer::new(header.length as usize))
                                .await
                                .into_result()
                            {
                                Ok(data) => Some(data),
                                Err(e) => {
                                    error!("Failed to read extra data: {}", e);
                                    None
                                }
                            }
                        } else {
                            None
                        };

                        if header.msg_type == VideoData {
                            file.write(&*extra_data.clone().unwrap()).unwrap();
                            file.write("\n".as_ref()).unwrap();
                        }
                        let message = header.to_message(extra_data).unwrap();
                        tx.send(*message).unwrap();
                    }
                    Err(e) => {
                        error!("Error reading from device: {}", e);
                    }
                }
            }
        });
    }

    pub async fn start(&mut self, config: DongleConfig) -> Result<(), DriverError> {
        *self.error_count.lock().unwrap() = 0;
        use crate::sendable::*;

        self.send(Box::new(SendNumber::new(config.dpi, FileAddress::Dpi)))
            .await?;

        self.send(Box::new(SendOpen::new(config.clone()))).await?;
        self.send(Box::new(SendBoolean::new(
            config.night_mode,
            FileAddress::NightMode,
        )))
        .await?;
        self.send(Box::new(SendNumber::new(
            config.hand as u32,
            FileAddress::HandDriveMode,
        )))
        .await?;
        self.send(Box::new(SendBoolean::new(true, FileAddress::ChargeMode)))
            .await?;
        self.send(Box::new(SendString::new(
            config.box_name.clone(),
            FileAddress::BoxName,
        )))
        .await?;
        self.send(Box::new(SendBoxSettings::new(config.clone(), None)))
            .await?;
        self.send(Box::new(SendCommand { value: WifiEnable }))
            .await?;
        self.send(Box::new(SendCommand {
            value: match config.wifi_type {
                WifiType::Ghz5 => Wifi5g,
                WifiType::Ghz2_4 => Wifi24g,
            },
        }))
        .await?;
        self.send(Box::new(SendCommand {
            value: match config.mic_type {
                MicType::Box => BoxMic,
                MicType::Os => Mic,
            },
        }))
        .await?;
        self.send(Box::new(SendCommand {
            value: if config.audio_transfer_mode {
                AudioTransferOn
            } else {
                AudioTransferOff
            },
        }))
        .await?;

        // Schedule Wi-Fi connect after delay
        let tx = self.message_tx.clone();
        tokio::spawn(async move {
            time::sleep(Duration::from_secs(1)).await;
            tx.send(Box::new(SendCommand { value: WifiConnect }))
                .await
                .unwrap();
        });

        // Start heartbeat
        let tx = self.message_tx.clone();
        self.heartbeat_handle = Some(tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(2));
            loop {
                interval.tick().await;
                tx.send(Box::new(HeartBeat)).await.unwrap();
            }
        }));

        Ok(())
    }

    pub async fn close(&mut self) -> Result<(), DriverError> {
        if let Some(handle) = self.heartbeat_handle.take() {
            handle.abort();
        }

        self.device = None;
        self.in_ep = None;
        self.out_ep = None;

        Ok(())
    }

    pub async fn run(&mut self, message_tx: Sender<Message>) {
        self.read_loop(message_tx.clone()).await;

        while let Some(message) = self.message_rx.recv().await {
            if let Err(e) = self.send(message).await {
                error!("Error sending message: {}", e);
            }
        }
    }
}
