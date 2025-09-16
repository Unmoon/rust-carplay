use crate::commands::CommandMapping;
use crate::message::MessageHeader;
use byteorder::{LittleEndian, ReadBytesExt};
use log::info;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum AudioCommand {
    AudioOutputStart = 1,
    AudioOutputStop = 2,
    AudioInputConfig = 3,
    AudioPhonecallStart = 4,
    AudioPhonecallStop = 5,
    AudioNaviStart = 6,
    AudioNaviStop = 7,
    AudioSiriStart = 8,
    AudioSiriStop = 9,
    AudioMediaStart = 10,
    AudioMediaStop = 11,
    AudioAlertStart = 12,
    AudioAlertStop = 13,
}

pub trait ReadableMessage {
    fn get_data(&self) -> Vec<u8> {
        Vec::new()
    }
}

#[derive(Debug, Clone)]
pub struct Command {
    pub header: MessageHeader,
    pub value: CommandMapping,
}

impl ReadableMessage for Command {}
impl Command {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        let mut cursor = Cursor::new(data);
        Command {
            header,
            value: CommandMapping::from(cursor.read_u32::<LittleEndian>().unwrap()),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ManufacturerInfo {
    pub header: MessageHeader,
    pub a: u32,
    pub b: u32,
}

impl ReadableMessage for ManufacturerInfo {}
impl ManufacturerInfo {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        let mut cursor = Cursor::new(data);
        let a = cursor.read_u32::<LittleEndian>().unwrap();
        let b = cursor.read_u32::<LittleEndian>().unwrap();
        ManufacturerInfo { header, a, b }
    }
}

#[derive(Debug, Clone)]
pub struct SoftwareVersion {
    pub header: MessageHeader,
    pub version: String,
}

impl ReadableMessage for SoftwareVersion {}
impl SoftwareVersion {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        let version = String::from_utf8_lossy(&*data).into_owned();
        SoftwareVersion { header, version }
    }
}

#[derive(Debug, Clone)]
pub struct BluetoothAddress {
    pub header: MessageHeader,
    pub address: String,
}

impl ReadableMessage for BluetoothAddress {}
impl BluetoothAddress {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        let address = String::from_utf8_lossy(&*data).into_owned();
        BluetoothAddress { header, address }
    }
}

#[derive(Debug, Clone)]
pub struct BluetoothPIN {
    pub header: MessageHeader,
    pub pin: String,
}

impl ReadableMessage for BluetoothPIN {}
impl BluetoothPIN {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        let pin = String::from_utf8_lossy(&*data).into_owned();
        BluetoothPIN { header, pin }
    }
}

#[derive(Debug, Clone)]
pub struct BluetoothDeviceName {
    pub header: MessageHeader,
    pub name: String,
}

impl ReadableMessage for BluetoothDeviceName {}
impl BluetoothDeviceName {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        let name = String::from_utf8_lossy(&*data).into_owned();
        BluetoothDeviceName { header, name }
    }
}

#[derive(Debug, Clone)]
pub struct WifiDeviceName {
    pub header: MessageHeader,
    pub name: String,
}

impl ReadableMessage for WifiDeviceName {}
impl WifiDeviceName {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        let name = String::from_utf8_lossy(&*data).into_owned();
        WifiDeviceName { header, name }
    }
}

#[derive(Debug, Clone)]
pub struct HiCarLink {
    pub header: MessageHeader,
    pub link: String,
}

impl ReadableMessage for HiCarLink {}
impl HiCarLink {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        let link = String::from_utf8_lossy(&*data).into_owned();
        HiCarLink { header, link }
    }
}

#[derive(Debug, Clone)]
pub struct BluetoothPairedList {
    pub header: MessageHeader,
    pub data: String,
}

impl ReadableMessage for BluetoothPairedList {}
impl BluetoothPairedList {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        let data = String::from_utf8_lossy(&*data).into_owned();
        BluetoothPairedList { header, data }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum PhoneType {
    AndroidMirror = 1,
    CarPlay = 3,
    IphoneMirror = 4,
    AndroidAuto = 5,
    HiCar = 6,
    Unknown = 255,
}

impl From<u32> for PhoneType {
    fn from(value: u32) -> Self {
        use PhoneType::*;
        match value {
            1 => AndroidMirror,
            3 => CarPlay,
            4 => IphoneMirror,
            5 => AndroidAuto,
            6 => HiCar,
            _ => Unknown,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Plugged {
    pub header: MessageHeader,
    pub phone_type: PhoneType,
    pub wifi: Option<u32>,
}

impl ReadableMessage for Plugged {}
impl Plugged {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        let data_len = data.len();
        let mut cursor = Cursor::new(data);
        let phone_type = PhoneType::from(cursor.read_u32::<LittleEndian>().unwrap());
        let wifi = if data_len == 8 {
            Some(cursor.read_u32::<LittleEndian>().unwrap())
        } else {
            None
        };

        info!(
            "{} avail, phone type: {:?}, wifi: {:?}",
            if wifi.is_some() { "wifi" } else { "no wifi" },
            phone_type,
            wifi
        );

        Plugged {
            header,
            phone_type,
            wifi,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Unplugged {
    pub header: MessageHeader,
}

impl ReadableMessage for Unplugged {}
impl Unplugged {
    pub fn new(header: MessageHeader) -> Self {
        Unplugged { header }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u8,
    pub bit_depth: u8,
}

lazy_static::lazy_static! {
    pub static ref DECODE_TYPE_MAP: HashMap<u32, AudioFormat> = {
        let mut m = HashMap::new();
        m.insert(1, AudioFormat {
            sample_rate: 44100,
            channels: 2,
            bit_depth: 16,
        });
        m.insert(2, AudioFormat {
            sample_rate: 44100,
            channels: 2,
            bit_depth: 16,
        });
        m.insert(3, AudioFormat {
            sample_rate: 8000,
            channels: 1,
            bit_depth: 16,
        });
        m.insert(4, AudioFormat {
            sample_rate: 48000,
            channels: 2,
            bit_depth: 16,
        });
        m.insert(5, AudioFormat {
            sample_rate: 16000,
            channels: 1,
            bit_depth: 16,
        });
        m.insert(6, AudioFormat {
            sample_rate: 24000,
            channels: 1,
            bit_depth: 16,
        });
        m.insert(7, AudioFormat {
            sample_rate: 16000,
            channels: 2,
            bit_depth: 16,
        });
        m
    };
}

#[derive(Debug, Clone)]
pub struct AudioData {
    pub header: MessageHeader,
    pub command: Option<AudioCommand>,
    pub decode_type: u32,
    pub volume: f32,
    pub volume_duration: Option<f32>,
    pub audio_type: u32,
    pub data: Option<Vec<i16>>,
}

impl ReadableMessage for AudioData {}
impl AudioData {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        let data_len = data.len();
        let mut cursor = Cursor::new(data);
        let decode_type = cursor.read_u32::<LittleEndian>().unwrap();
        let volume = cursor.read_f32::<LittleEndian>().unwrap();
        let audio_type = cursor.read_u32::<LittleEndian>().unwrap();

        let amount = data_len - 12;
        let (command, volume_duration, data) = if amount == 1 {
            let command_val = cursor.read_i8().unwrap();
            (
                Some(unsafe { std::mem::transmute(command_val as u32) }),
                None,
                None,
            )
        } else if amount == 4 {
            (None, Some(cursor.read_f32::<LittleEndian>().unwrap()), None)
        } else {
            let mut audio_data = Vec::with_capacity(amount / 2);
            for _ in 0..(amount / 2) {
                audio_data.push(cursor.read_i16::<LittleEndian>().unwrap());
            }
            (None, None, Some(audio_data))
        };

        AudioData {
            header,
            command,
            decode_type,
            volume,
            volume_duration,
            audio_type,
            data,
        }
    }

    pub fn get_audio_format(&self) -> Option<&AudioFormat> {
        DECODE_TYPE_MAP.get(&self.decode_type)
    }
}

#[derive(Debug, Clone)]
pub struct VideoData {
    pub header: MessageHeader,
    pub width: u32,
    pub height: u32,
    pub flags: u32,
    pub length: u32,
    pub unknown: u32,
    pub data: Vec<u8>,
}

impl ReadableMessage for VideoData {
    fn get_data(&self) -> Vec<u8> {
        self.data.clone()
    }
}
impl VideoData {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        // TODO: 20 or 21?
        let mut cursor = Cursor::new(data[..20].to_vec());
        let width = cursor.read_u32::<LittleEndian>().unwrap();
        let height = cursor.read_u32::<LittleEndian>().unwrap();
        let flags = cursor.read_u32::<LittleEndian>().unwrap();
        let length = cursor.read_u32::<LittleEndian>().unwrap();
        let unknown = cursor.read_u32::<LittleEndian>().unwrap();
        let data = data[20..].to_vec();

        VideoData {
            header,
            width,
            height,
            flags,
            length,
            unknown,
            data,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MediaType {
    Data = 1,
    AlbumCover = 3,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaInfo {
    #[serde(rename = "MediaSongName")]
    pub media_song_name: Option<String>,
    #[serde(rename = "MediaAlbumName")]
    pub media_album_name: Option<String>,
    #[serde(rename = "MediaArtistName")]
    pub media_artist_name: Option<String>,
    #[serde(rename = "MediaAPPName")]
    pub media_app_name: Option<String>,
    #[serde(rename = "MediaSongDuration")]
    pub media_song_duration: Option<f64>,
    #[serde(rename = "MediaSongPlayTime")]
    pub media_song_play_time: Option<f64>,
}

#[derive(Debug, Clone)]
pub enum MediaPayload {
    Data { media: MediaInfo },
    AlbumCover { base64_image: String },
}

#[derive(Debug, Clone)]
pub struct MediaData {
    pub header: MessageHeader,
    pub payload: Option<MediaPayload>,
}

impl ReadableMessage for MediaData {}
impl MediaData {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        use base64::{engine::general_purpose, Engine as _};
        let data_len = data.len();
        // TODO: is 4 correct?
        let mut cursor = Cursor::new(data[..4].to_vec());
        let type_val = cursor.read_u32::<LittleEndian>().unwrap();

        let payload = match type_val {
            1 => {
                let media_data = &data[4..data_len - 1];
                if let Ok(media) = serde_json::from_slice::<MediaInfo>(media_data) {
                    Some(MediaPayload::Data { media })
                } else {
                    None
                }
            }
            3 => {
                let image_data = &data[4..];
                Some(MediaPayload::AlbumCover {
                    base64_image: general_purpose::STANDARD.encode(image_data),
                })
            }
            _ => {
                println!("Unexpected media type: {}", type_val);
                None
            }
        };

        MediaData { header, payload }
    }
}

#[derive(Debug, Clone)]
pub struct Opened {
    pub header: MessageHeader,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub format: u32,
    pub packet_max: u32,
    pub i_box: u32,
    pub phone_mode: u32,
}

impl ReadableMessage for Opened {}
impl Opened {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        let mut cursor = Cursor::new(data);
        let width = cursor.read_u32::<LittleEndian>().unwrap();
        let height = cursor.read_u32::<LittleEndian>().unwrap();
        let fps = cursor.read_u32::<LittleEndian>().unwrap();
        let format = cursor.read_u32::<LittleEndian>().unwrap();
        let packet_max = cursor.read_u32::<LittleEndian>().unwrap();
        let i_box = cursor.read_u32::<LittleEndian>().unwrap();
        let phone_mode = cursor.read_u32::<LittleEndian>().unwrap();
        Opened {
            header,
            width,
            height,
            fps,
            format,
            packet_max,
            i_box,
            phone_mode,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BoxSettings {
    Type1 {
        #[serde(rename = "HiCar")]
        hi_car: u32,
        #[serde(rename = "OemName")]
        oem_name: String,
        #[serde(rename = "WiFiChannel")]
        wifi_channel: u32,
        #[serde(rename = "boxType")]
        box_type: String,
        #[serde(rename = "hwVersion")]
        hw_version: String,
        #[serde(rename = "productType")]
        product_type: String,
        uuid: String,
    },
    Type2 {
        #[serde(rename = "MDLinkType")]
        md_link_type: String,
        #[serde(rename = "MDModel")]
        md_model: String,
        #[serde(rename = "MDOSVersion")]
        md_os_version: String,
        #[serde(rename = "MDLinkVersion")]
        md_link_version: String,
        #[serde(rename = "cpuTemp")]
        cpu_temp: f64,
    },
}

#[derive(Debug, Clone)]
pub struct BoxInfo {
    pub header: MessageHeader,
    pub settings: BoxSettings,
}

impl ReadableMessage for BoxInfo {}
impl BoxInfo {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        let data_string = String::from_utf8(data).unwrap();
        let settings = serde_json::from_str(&*data_string).unwrap();

        BoxInfo { header, settings }
    }
}

#[derive(Debug, Clone)]
pub struct Phase {
    pub header: MessageHeader,
    pub phase: u32,
}

impl ReadableMessage for Phase {}
impl Phase {
    pub fn new(header: MessageHeader, data: Vec<u8>) -> Self {
        let mut cursor = Cursor::new(data);
        let phase = cursor.read_u32::<LittleEndian>().unwrap();
        Phase { header, phase }
    }
}

#[derive(Debug, Clone)]
pub struct Unknown {
    pub header: MessageHeader,
}

impl ReadableMessage for Unknown {}
impl Unknown {
    pub fn new(header: MessageHeader) -> Self {
        Unknown { header }
    }
}
