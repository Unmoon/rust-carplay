
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandMapping {
    Invalid = 0,
    StartRecordAudio = 1, 
    StopRecordAudio = 2, 
    RequestHostUI = 3, // "My Car" button clicked in the Carplay interface
    Siri = 5, // Siri Button
    Mic = 7, // Car Microphone
    Frame = 12,
    BoxMic = 15, // Box Microphone
    EnableNightMode = 16, // Enable night mode
    DisableNightMode = 17, // Disable night mode
    AudioTransferOn = 22, // Phone will stream audio directly to car system and not dongle
    AudioTransferOff = 23, // DEFAULT - Phone will stream audio to the dongle, and it will send it over the link
    Wifi24g = 24, // 2.4G Wifi
    Wifi5g = 25, // 5G Wifi
    Left = 100, // Button Left
    Right = 101, // Button Right
    SelectDown = 104, // Button Select Down
    SelectUp = 105, // Button Select Up
    Back = 106, // Button Back
    Up = 113, // Button Up
    Down = 114, // Button Down
    Home = 200, // Button Home
    Play = 201, // Button Play
    Pause = 202, // Button Pause
    PlayOrPause = 203, // Button Switch Play/Pause
    Next = 204, // Button Next Track
    Prev = 205, // Button Prev Track
    AcceptPhone = 300, // Accept Phone Call
    RejectPhone = 301, // Reject Phone Call
    RequestVideoFocus = 500,
    ReleaseVideoFocus = 501,
    WifiEnable = 1000,
    AutoConnectEnable = 1001,
    WifiConnect = 1002,
    ScanningDevice = 1003,
    DeviceFound = 1004,
    DeviceNotFound = 1005,
    ConnectDeviceFailed = 1006,
    BtConnected = 1007,
    BtDisconnected = 1008,
    WifiConnected = 1009,
    WifiDisconnected = 1010,
    BtPairStart = 1011,
    WifiPair = 1012,
}

impl From<u32> for CommandMapping {
    fn from(value: u32) -> Self {
        use CommandMapping::*;
        match value {
            0 => Invalid,
            1 => StartRecordAudio,
            2 => StopRecordAudio,
            3 => RequestHostUI,
            5 => Siri,
            7 => Mic,
            15 => BoxMic,
            16 => EnableNightMode,
            17 => DisableNightMode,
            24 => Wifi24g,
            25 => Wifi5g,
            100 => Left,
            101 => Right,
            12 => Frame,
            22 => AudioTransferOn,
            23 => AudioTransferOff,
            104 => SelectDown,
            105 => SelectUp,
            106 => Back,
            113 => Up,
            114 => Down,
            200 => Home,
            201 => Play,
            202 => Pause,
            203 => PlayOrPause,
            204 => Next,
            205 => Prev,
            300 => AcceptPhone,
            301 => RejectPhone,
            500 => RequestVideoFocus,
            501 => ReleaseVideoFocus,
            1000 => WifiEnable,
            1001 => AutoConnectEnable,
            1002 => WifiConnect,
            1003 => ScanningDevice,
            1004 => DeviceFound,
            1005 => DeviceNotFound,
            1006 => ConnectDeviceFailed,
            1007 => BtConnected,
            1008 => BtDisconnected,
            1009 => WifiConnected,
            1010 => WifiDisconnected,
            1011 => BtPairStart,
            1012 => WifiPair,
            _ => Invalid, // fallback for unknown values
        }
    }
}

impl From<CommandMapping> for u32 {
    fn from(cmd: CommandMapping) -> u32 {
        cmd as u32
    }
}
