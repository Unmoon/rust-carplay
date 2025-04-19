use crate::messagetypes;
use crate::messagetypes::MessageType::Open;
use crate::readable::*;
use crate::sendable::*;
use byteorder::{ByteOrder, LittleEndian};
use log::warn;
use std::fmt;

const HEADER_SIZE: usize = 16;
const MAGIC: u32 = 0x55AA55AA;

#[derive(Debug, Clone)]
pub enum Message {
    SendOpen(SendOpen),
    ReadOpen(Opened),
    ReadPlugged(Plugged),
    ReadPhase(Phase),
    ReadUnplugged(Unplugged),
    SendTouch(SendTouch),
    ReadVideoData(VideoData),
    ReadAudioData(AudioData),
    SendCommand(SendCommand),
    ReadCommand(Command),
    SendLogoType(SendLogoType),
    ReadBluetoothAddress(BluetoothAddress),
    ReadBluetoothPIN(BluetoothPIN),
    ReadBluetoothDeviceName(BluetoothDeviceName),
    ReadWifiDeviceName(WifiDeviceName),
    SendDisconnectPhone(SendDisconnectPhone),
    ReadBluetoothPairedList(BluetoothPairedList),
    ReadManufacturerInfo(ManufacturerInfo),
    SendCloseDongle(SendCloseDongle),
    SendMultiTouch(SendMultiTouch),
    ReadHiCarLink(HiCarLink),
    SendBoxSettings(SendBoxSettings),
    ReadBoxSettings(BoxInfo),
    ReadMediaData(MediaData),
    ReadFile(SendFile),
    SendFile(SendFile),
    ReadHeartBeat(HeartBeat),
    ReadSoftwareVersion(SoftwareVersion),
    ReadUnknown(Unknown),
}

#[derive(Debug, Clone)]
pub struct MessageHeader {
    pub length: u32,
    pub msg_type: crate::messagetypes::MessageType,
}

#[derive(Debug)]
pub enum HeaderBuildError {
    InvalidSize(usize),
    InvalidMagic(u32),
    InvalidTypeCheck { expected: u32, actual: u32 },
}

impl fmt::Display for HeaderBuildError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            HeaderBuildError::InvalidSize(size) => write!(f, "Invalid buffer size: {}", size),
            HeaderBuildError::InvalidMagic(magic) => write!(f, "Invalid magic number: {}", magic),
            HeaderBuildError::InvalidTypeCheck { expected, actual } => write!(
                f,
                "Invalid type check: expected {:08X}, got {:08X}",
                expected, actual
            ),
        }
    }
}

impl MessageHeader {
    pub fn from_bytes(data: &[u8]) -> Result<Self, HeaderBuildError> {
        if data.len() != HEADER_SIZE {
            return Err(HeaderBuildError::InvalidSize(data.len()));
        }

        let magic = LittleEndian::read_u32(&data[0..4]);
        if magic != MAGIC {
            return Err(HeaderBuildError::InvalidMagic(magic));
        }

        let length = LittleEndian::read_u32(&data[4..8]);
        let type_raw = LittleEndian::read_u32(&data[8..12]);
        let type_check = LittleEndian::read_u32(&data[12..16]);

        let expected_check = (!type_raw) & 0xFFFFFFFF;

        if type_check != expected_check {
            return Err(HeaderBuildError::InvalidTypeCheck {
                expected: expected_check,
                actual: type_check,
            });
        }

        Ok(Self {
            length,
            msg_type: crate::messagetypes::MessageType::from(type_raw as u8),
        })
    }

    pub fn to_bytes(&self) -> [u8; 16] {
        let mut buffer = [0u8; 16];
        LittleEndian::write_u32(&mut buffer[0..4], MAGIC);
        LittleEndian::write_u32(&mut buffer[4..8], self.length);
        LittleEndian::write_u32(&mut buffer[8..12], self.msg_type.into());
        let check = (!u32::from(self.msg_type)) & 0xFFFFFFFF;
        LittleEndian::write_u32(&mut buffer[12..16], check);
        buffer
    }

    pub fn to_message(&self, data: Option<Vec<u8>>) -> Result<Box<Message>, HeaderBuildError> {
        use crate::readable::*;
        use crate::sendable::*;

        match (self.msg_type, data) {
            (messagetypes::MessageType::Command, Some(d)) => Ok(Box::new(Message::ReadCommand(
                Command::new(self.clone(), d),
            ))),
            (messagetypes::MessageType::ManufacturerInfo, Some(d)) => Ok(Box::new(
                Message::ReadManufacturerInfo(ManufacturerInfo::new(self.clone(), d)),
            )),
            (messagetypes::MessageType::SoftwareVersion, Some(d)) => Ok(Box::new(
                Message::ReadSoftwareVersion(SoftwareVersion::new(self.clone(), d)),
            )),
            (messagetypes::MessageType::BluetoothAddress, Some(d)) => Ok(Box::new(
                Message::ReadBluetoothAddress(BluetoothAddress::new(self.clone(), d)),
            )),
            (messagetypes::MessageType::BluetoothPIN, Some(d)) => Ok(Box::new(
                Message::ReadBluetoothPIN(BluetoothPIN::new(self.clone(), d)),
            )),
            (messagetypes::MessageType::BluetoothDeviceName, Some(d)) => Ok(Box::new(
                Message::ReadBluetoothDeviceName(BluetoothDeviceName::new(self.clone(), d)),
            )),
            (messagetypes::MessageType::WifiDeviceName, Some(d)) => Ok(Box::new(
                Message::ReadWifiDeviceName(WifiDeviceName::new(self.clone(), d)),
            )),
            (messagetypes::MessageType::HiCarLink, Some(d)) => Ok(Box::new(
                Message::ReadHiCarLink(HiCarLink::new(self.clone(), d)),
            )),
            (messagetypes::MessageType::BluetoothPairedList, Some(d)) => Ok(Box::new(
                Message::ReadBluetoothPairedList(BluetoothPairedList::new(self.clone(), d)),
            )),
            (messagetypes::MessageType::Plugged, Some(d)) => Ok(Box::new(Message::ReadPlugged(
                Plugged::new(self.clone(), d),
            ))),
            (messagetypes::MessageType::AudioData, Some(d)) => Ok(Box::new(
                Message::ReadAudioData(AudioData::new(self.clone(), d)),
            )),
            (messagetypes::MessageType::VideoData, Some(d)) => Ok(Box::new(
                Message::ReadVideoData(VideoData::new(self.clone(), d)),
            )),
            (messagetypes::MessageType::MediaData, Some(d)) => Ok(Box::new(
                Message::ReadMediaData(MediaData::new(self.clone(), d)),
            )),
            (messagetypes::MessageType::BoxSettings, Some(d)) => Ok(Box::new(
                Message::ReadBoxSettings(BoxInfo::new(self.clone(), d)),
            )),
            (messagetypes::MessageType::Phase, Some(d)) => {
                Ok(Box::new(Message::ReadPhase(Phase::new(self.clone(), d))))
            }
            (messagetypes::MessageType::Unplugged, None) => Ok(Box::new(Message::ReadUnplugged(
                Unplugged::new(self.clone()),
            ))),
            (Open, Some(d)) => Ok(Box::new(Message::ReadOpen(Opened::new(self.clone(), d)))),
            (messagetypes::MessageType::Touch, None) => {
                // TODO
                Ok(Box::new(Message::SendTouch(SendTouch::new(
                    0.0,
                    0.0,
                    TouchAction::Down,
                ))))
            }
            (messagetypes::MessageType::LogoType, None) => Ok(Box::new(Message::SendLogoType(
                SendLogoType::new(LogoType::HomeButton),
            ))),
            (messagetypes::MessageType::DisconnectPhone, None) => Ok(Box::new(
                Message::SendDisconnectPhone(SendDisconnectPhone {}),
            )),
            (messagetypes::MessageType::CloseDongle, None) => {
                Ok(Box::new(Message::SendCloseDongle(SendCloseDongle {})))
            }
            (messagetypes::MessageType::MultiTouch, None) => {
                // TODO
                Ok(Box::new(Message::SendMultiTouch(SendMultiTouch::new(
                    Vec::new(),
                ))))
            }
            (messagetypes::MessageType::SendFile, Some(d)) => {
                // TODO
                Ok(Box::new(Message::SendFile(SendFile::new(
                    d,
                    String::from(""),
                ))))
            }
            (messagetypes::MessageType::HeartBeat, None) => {
                Ok(Box::new(Message::ReadHeartBeat(HeartBeat {})))
            }
            (messagetypes::MessageType::Unknown(_), None) => {
                warn!("Unknown message type: {:?}", self.msg_type);
                Ok(Box::new(Message::ReadUnknown(Unknown::new(self.clone()))))
            }
            _ => {
                warn!("Unknown message type: {:?}", self.msg_type);
                Ok(Box::new(Message::ReadUnknown(Unknown::new(self.clone()))))
            }
        }
    }
}
