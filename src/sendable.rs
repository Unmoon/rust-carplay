use crate::commands::CommandMapping;
use crate::driver::DongleConfig;
use crate::message::MessageHeader;
use crate::messagetypes::MessageType;
use byteorder::{LittleEndian, WriteBytesExt};
use futures::AsyncWriteExt;
use futures_lite::future::block_on;
use log::error;
use serde::Serialize;
use std::time::{SystemTime, UNIX_EPOCH};

pub trait SendableMessage {
    fn message_type(&self) -> MessageType;
    fn get_payload(&self) -> Vec<u8>;
    fn serialize(&self) -> Vec<u8> {
        let data = self.get_payload();
        let message_header = MessageHeader {
            msg_type: self.message_type(),
            length: data.len() as u32,
        };
        let mut header = Vec::with_capacity(8);
        header.extend(message_header.to_bytes());
        header.extend(data);
        // info!("msgtype: {:?}, header: {:?}", self.message_type(), header);
        header
    }
}

#[derive(Clone, Debug)]
pub struct SendCommand {
    pub value: CommandMapping,
}

impl SendCommand {
    pub fn new(value: u32) -> Self {
        Self {
            value: CommandMapping::from(value),
        }
    }
}

impl SendableMessage for SendCommand {
    fn message_type(&self) -> MessageType {
        MessageType::Command
    }
    fn get_payload(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4);
        buf.write_u32::<LittleEndian>(self.value.into()).unwrap();
        buf
    }
}

#[derive(Debug, Clone, Copy)]
pub enum TouchAction {
    Down = 14,
    Move = 15,
    Up = 16,
}

#[derive(Clone, Debug)]
pub struct SendTouch {
    pub x: f32,
    pub y: f32,
    pub action: TouchAction,
}

impl SendTouch {
    pub fn new(x: f32, y: f32, action: TouchAction) -> Self {
        Self { x, y, action }
    }

    fn clamp(value: f32, min: f32, max: f32) -> f32 {
        if value < min {
            min
        } else if value > max {
            max
        } else {
            value
        }
    }
}

impl SendableMessage for SendTouch {
    fn message_type(&self) -> MessageType {
        MessageType::Touch
    }
    fn get_payload(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(16);

        // Action
        buf.write_u32::<LittleEndian>(self.action as u32).unwrap();

        // X and Y coordinates
        let final_x = Self::clamp(10000.0 * self.x, 0.0, 10000.0);
        let final_y = Self::clamp(10000.0 * self.y, 0.0, 10000.0);
        buf.write_u32::<LittleEndian>(final_x as u32).unwrap();
        buf.write_u32::<LittleEndian>(final_y as u32).unwrap();

        // Flags (empty)
        buf.write_u32::<LittleEndian>(0).unwrap();

        buf
    }
}

#[derive(Debug, Clone, Copy)]
pub enum MultiTouchAction {
    Down = 1,
    Move = 2,
    Up = 0,
}

#[derive(Debug, Clone, Copy)]
struct TouchItem {
    x: f32,
    y: f32,
    action: MultiTouchAction,
    id: u32,
}

impl TouchItem {
    fn new(x: f32, y: f32, action: MultiTouchAction, id: u32) -> Self {
        Self { x, y, action, id }
    }

    fn get_payload(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(16);
        buf.write_f32::<LittleEndian>(self.x).unwrap();
        buf.write_f32::<LittleEndian>(self.y).unwrap();
        buf.write_u32::<LittleEndian>(self.action as u32).unwrap();
        buf.write_u32::<LittleEndian>(self.id).unwrap();
        buf
    }
}

#[derive(Clone, Debug)]
pub struct SendMultiTouch {
    touches: Vec<TouchItem>,
}

impl SendMultiTouch {
    pub fn new(touch_data: Vec<(f32, f32, MultiTouchAction)>) -> Self {
        let touches = touch_data
            .into_iter()
            .enumerate()
            .map(|(index, (x, y, action))| TouchItem::new(x, y, action, index as u32))
            .collect();
        Self { touches }
    }
}

impl SendableMessage for SendMultiTouch {
    fn message_type(&self) -> MessageType {
        MessageType::MultiTouch
    }
    fn get_payload(&self) -> Vec<u8> {
        self.touches
            .iter()
            .flat_map(|item| item.get_payload())
            .collect()
    }
}

pub struct SendAudio {
    data: Vec<i16>,
}

impl SendAudio {
    pub fn new(data: Vec<i16>) -> Self {
        Self { data }
    }
}

impl SendableMessage for SendAudio {
    fn message_type(&self) -> MessageType {
        MessageType::AudioData
    }
    fn get_payload(&self) -> Vec<u8> {
        let mut audio_data = Vec::with_capacity(12 + self.data.len() * 2);
        audio_data.write_u32::<LittleEndian>(5).unwrap(); // decode_type
        audio_data.write_f32::<LittleEndian>(0.0).unwrap(); // volume
        audio_data.write_u32::<LittleEndian>(3).unwrap(); // audio_type

        // Convert i16 samples to bytes
        for &sample in &self.data {
            audio_data.write_i16::<LittleEndian>(sample).unwrap();
        }

        audio_data
    }
}

#[derive(Clone, Debug)]
pub struct SendFile {
    content: Vec<u8>,
    file_name: String,
}

impl SendFile {
    pub fn new(content: Vec<u8>, file_name: String) -> Self {
        Self { content, file_name }
    }

    fn get_file_name(&self) -> Vec<u8> {
        let mut name = self.file_name.clone();
        name.push('\0');
        name.into_bytes()
    }

    fn get_length(data: &[u8]) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4);
        buf.write_u32::<LittleEndian>(data.len() as u32).unwrap();
        buf
    }
}

impl SendableMessage for SendFile {
    fn message_type(&self) -> MessageType {
        MessageType::SendFile
    }
    fn get_payload(&self) -> Vec<u8> {
        let new_file_name = self.get_file_name();
        let content_length = Self::get_length(&self.content);
        let mut buf = Vec::new();
        buf.write_u32::<LittleEndian>(new_file_name.len() as u32)
            .unwrap();
        block_on(buf.write_all(&*new_file_name)).unwrap();
        buf.write_u32::<LittleEndian>(content_length.len() as u32)
            .unwrap();
        block_on(buf.write_all(&self.content)).unwrap();
        buf
    }
}

#[derive(Debug, Clone)]
pub enum FileAddress {
    Dpi,
    NightMode,
    HandDriveMode,
    ChargeMode,
    BoxName,
    OemIcon,
    AirplayConfig,
    Icon120,
    Icon180,
    Icon250,
    AndroidWorkMode,
}

impl FileAddress {
    fn as_str(&self) -> &'static str {
        match self {
            FileAddress::Dpi => "/tmp/screen_dpi",
            FileAddress::NightMode => "/tmp/night_mode",
            FileAddress::HandDriveMode => "/tmp/hand_drive_mode",
            FileAddress::ChargeMode => "/tmp/charge_mode",
            FileAddress::BoxName => "/etc/box_name",
            FileAddress::OemIcon => "/etc/oem_icon.png",
            FileAddress::AirplayConfig => "/etc/airplay.conf",
            FileAddress::Icon120 => "/etc/icon_120x120.png",
            FileAddress::Icon180 => "/etc/icon_180x180.png",
            FileAddress::Icon250 => "/etc/icon_256x256.png",
            FileAddress::AndroidWorkMode => "/etc/android_work_mode",
        }
    }
}

pub struct SendNumber {
    inner: SendFile,
}

impl SendNumber {
    pub fn new(content: u32, file: FileAddress) -> Self {
        let mut message = Vec::with_capacity(4);
        message.write_u32::<LittleEndian>(content).unwrap();
        let inner = SendFile::new(message, file.as_str().to_string());
        Self { inner }
    }
}

impl SendableMessage for SendNumber {
    fn message_type(&self) -> MessageType {
        self.inner.message_type()
    }
    fn get_payload(&self) -> Vec<u8> {
        self.inner.get_payload()
    }
}

pub struct SendBoolean {
    inner: SendNumber,
}

impl SendBoolean {
    pub fn new(content: bool, file: FileAddress) -> Self {
        let inner = SendNumber::new(content as u32, file);
        Self { inner }
    }
}

impl SendableMessage for SendBoolean {
    fn message_type(&self) -> MessageType {
        self.inner.message_type()
    }
    fn get_payload(&self) -> Vec<u8> {
        self.inner.get_payload()
    }
}

pub struct SendString {
    inner: SendFile,
}

impl SendString {
    pub fn new(content: String, file: FileAddress) -> Self {
        if content.len() > 16 {
            error!("string too long");
        }
        let message = content.into_bytes();
        let inner = SendFile::new(message, file.as_str().to_string());
        Self { inner }
    }
}

impl SendableMessage for SendString {
    fn message_type(&self) -> MessageType {
        self.inner.message_type()
    }
    fn get_payload(&self) -> Vec<u8> {
        self.inner.get_payload()
    }
}

#[derive(Clone, Debug)]
pub struct HeartBeat;

impl SendableMessage for HeartBeat {
    fn message_type(&self) -> MessageType {
        MessageType::HeartBeat
    }
    fn get_payload(&self) -> Vec<u8> {
        Vec::new()
    }
}

#[derive(Clone, Debug)]
pub struct SendOpen {
    config: DongleConfig,
}

impl SendOpen {
    pub fn new(config: DongleConfig) -> Self {
        Self { config }
    }
}

impl SendableMessage for SendOpen {
    fn message_type(&self) -> MessageType {
        MessageType::Open
    }
    fn get_payload(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(28);
        buf.write_u32::<LittleEndian>(self.config.width).unwrap();
        buf.write_u32::<LittleEndian>(self.config.height).unwrap();
        buf.write_u32::<LittleEndian>(self.config.fps).unwrap();
        buf.write_u32::<LittleEndian>(self.config.format).unwrap();
        buf.write_u32::<LittleEndian>(self.config.packet_max)
            .unwrap();
        buf.write_u32::<LittleEndian>(self.config.i_box_version)
            .unwrap();
        buf.write_u32::<LittleEndian>(self.config.phone_work_mode)
            .unwrap();
        buf
    }
}

#[derive(Clone, Debug)]
pub struct SendBoxSettings {
    sync_time: Option<u64>,
    config: DongleConfig,
}

impl SendBoxSettings {
    pub fn new(config: DongleConfig, sync_time: Option<u64>) -> Self {
        Self { config, sync_time }
    }

    fn get_current_time_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }
}

impl SendableMessage for SendBoxSettings {
    fn message_type(&self) -> MessageType {
        MessageType::BoxSettings
    }
    fn get_payload(&self) -> Vec<u8> {
        #[derive(Serialize)]
        struct BoxSettingsPayload {
            media_delay: u32,
            sync_time: u64,
            android_auto_size_w: u32,
            android_auto_size_h: u32,
        }

        let payload = BoxSettingsPayload {
            media_delay: self.config.media_delay,
            sync_time: self.sync_time.unwrap_or_else(Self::get_current_time_ms),
            android_auto_size_w: self.config.width,
            android_auto_size_h: self.config.height,
        };

        serde_json::to_vec(&payload).unwrap()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LogoType {
    HomeButton = 1,
    Siri = 2,
}

#[derive(Clone, Debug)]
pub struct SendLogoType {
    logo_type: LogoType,
}

impl SendLogoType {
    pub fn new(logo_type: LogoType) -> Self {
        Self { logo_type }
    }
}

impl SendableMessage for SendLogoType {
    fn message_type(&self) -> MessageType {
        MessageType::LogoType
    }
    fn get_payload(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(4);
        buf.write_u32::<LittleEndian>(self.logo_type as u32)
            .unwrap();
        buf
    }
}

pub struct SendIconConfig {
    inner: SendFile,
}

impl SendIconConfig {
    pub fn new(config: IconConfig) -> Self {
        let mut value_map = vec![
            ("oemIconVisible", "1"),
            ("name", "AutoBox"),
            ("model", "Magic-Car-Link-1.00"),
            ("oemIconPath", FileAddress::OemIcon.as_str()),
        ];

        if let Some(label) = config.label {
            value_map.push(("oemIconLabel", label));
        }

        let file_data = value_map
            .iter()
            .map(|(k, v)| format!("{} = {}", k, v))
            .collect::<Vec<_>>()
            .join("\n");

        let inner = SendFile::new(
            format!("{}\n", file_data).into_bytes(),
            FileAddress::AirplayConfig.as_str().to_string(),
        );

        Self { inner }
    }
}

impl SendableMessage for SendIconConfig {
    fn message_type(&self) -> MessageType {
        self.inner.message_type()
    }
    fn get_payload(&self) -> Vec<u8> {
        self.inner.get_payload()
    }
}

#[derive(Debug, Default)]
pub struct IconConfig {
    pub label: Option<&'static str>,
}

#[derive(Clone, Debug)]
pub struct SendCloseDongle;

impl SendableMessage for SendCloseDongle {
    fn message_type(&self) -> MessageType {
        MessageType::CloseDongle
    }
    fn get_payload(&self) -> Vec<u8> {
        Vec::new()
    }
}

#[derive(Clone, Debug)]
pub struct SendDisconnectPhone;

impl SendableMessage for SendDisconnectPhone {
    fn message_type(&self) -> MessageType {
        MessageType::DisconnectPhone
    }
    fn get_payload(&self) -> Vec<u8> {
        Vec::new()
    }
}
