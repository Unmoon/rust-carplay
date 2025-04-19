#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum MessageType {
    Open = 0x01,
    Plugged = 0x02,
    Phase = 0x03,
    Unplugged = 0x04,
    Touch = 0x05,
    VideoData = 0x06,
    AudioData = 0x07,
    Command = 0x08,
    LogoType = 0x09,
    DisconnectPhone = 0x0f,
    CloseDongle = 0x15,
    BluetoothAddress = 0x0a,
    BluetoothPIN = 0x0c,
    BluetoothDeviceName = 0x0d,
    WifiDeviceName = 0x0e,
    BluetoothPairedList = 0x12,
    ManufacturerInfo = 0x14,
    MultiTouch = 0x17,
    HiCarLink = 0x18,
    BoxSettings = 0x19,
    MediaData = 0x2a,
    SendFile = 0x99,
    HeartBeat = 0xaa,
    SoftwareVersion = 0xcc,
    Unknown(u8),
}

impl From<u8> for MessageType {
    fn from(value: u8) -> Self {
        use MessageType::*;
        match value {
            0x01 => Open,
            0x02 => Plugged,
            0x03 => Phase,
            0x04 => Unplugged,
            0x05 => Touch,
            0x06 => VideoData,
            0x07 => AudioData,
            0x08 => Command,
            0x09 => LogoType,
            0x0a => BluetoothAddress,
            0x0c => BluetoothPIN,
            0x0d => BluetoothDeviceName,
            0x0e => WifiDeviceName,
            0x0f => DisconnectPhone,
            0x12 => BluetoothPairedList,
            0x14 => ManufacturerInfo,
            0x15 => CloseDongle,
            0x17 => MultiTouch,
            0x18 => HiCarLink,
            0x19 => BoxSettings,
            0x2a => MediaData,
            0x99 => SendFile,
            0xaa => HeartBeat,
            0xcc => SoftwareVersion,
            other => Unknown(other),
        }
    }
}

impl From<MessageType> for u32 {
    fn from(msg: MessageType) -> u32 {
        use MessageType::*;
        match msg {
            Open => 0x01,
            Plugged => 0x02,
            Phase => 0x03,
            Unplugged => 0x04,
            Touch => 0x05,
            VideoData => 0x06,
            AudioData => 0x07,
            Command => 0x08,
            LogoType => 0x09,
            BluetoothAddress => 0x0a,
            BluetoothPIN => 0x0c,
            BluetoothDeviceName => 0x0d,
            WifiDeviceName => 0x0e,
            DisconnectPhone => 0x0f,
            BluetoothPairedList => 0x12,
            ManufacturerInfo => 0x14,
            CloseDongle => 0x15,
            MultiTouch => 0x17,
            HiCarLink => 0x18,
            BoxSettings => 0x19,
            MediaData => 0x2a,
            SendFile => 0x99,
            HeartBeat => 0xaa,
            SoftwareVersion => 0xcc,
            Unknown(code) => code as u32,
        }
    }
}
