use std::{fmt::Display, error::Error as StdError};

use nalgebra::{Point2, coordinates::XY};

#[derive(Clone, Debug)]
pub struct Packet {
    pub data: PacketData,
    pub id: u8,
}
#[derive(Clone, Debug)]
pub enum PacketData {
    WriteRegister(WriteRegister), // a.k.a. Poke
    ReadRegister(Register), // a.k.a. Peek
    ReadRegisterResponse(ReadRegisterResponse),
    WriteConfig(GeneralConfig),
    ReadConfig,
    ReadConfigResponse(GeneralConfig),
    ObjectReportRequest(ObjectReportRequest),
    ObjectReport(ObjectReport),
    CombinedMarkersReport(CombinedMarkersReport),
    AccelReport(AccelReport),
    ImpactReport,
    StreamUpdate(StreamUpdate),
    FlashSettings,
}
pub enum StreamChoice {
    Object,
    CombinedMarkers,
    Accel,
    Impact,
}

#[derive(Clone, Copy, Debug)]
pub struct Register {
    pub port: Port,
    pub bank: u8,
    pub address: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct WriteRegister {
    pub port: Port,
    pub bank: u8,
    pub address: u8,
    pub data: u8,
}

#[derive(Clone, Copy, Debug)]
pub struct ReadRegisterResponse {
    pub bank: u8,
    pub address: u8,
    pub data: u8,
}

#[derive(Copy, Clone, Debug, enumn::N, Default, PartialEq, Eq)]
pub enum MarkerPattern {
    #[default]
    Diamond,
    Rectangle,
}

impl MarkerPattern {
    // in the same order as the sort functions
    pub fn marker_positions(self) -> [Point2<f64>; 4] {
        match self {
            Self::Diamond => [
                [0.5, 1.0].into(), // bottom
                [0.0, 0.5].into(), // left
                [0.5, 0.0].into(), // top
                [1.0, 0.5].into(), // right
            ],
            Self::Rectangle => [
                [0.35, 0.0].into(), // top left
                [0.65, 0.0].into(), // top right
                [0.65, 1.0].into(), // bottom right
                [0.35, 1.0].into(), // bottom left
            ],
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct GeneralConfig {
    pub impact_threshold: u8,
    pub accel_odr: u16,
}

#[derive(Clone, Copy, Debug)]
pub struct ObjectReportRequest {}

#[derive(Clone, Copy, Debug, Default)]
pub struct MotData {
    pub area: u16,
    pub cx: u16,
    pub cy: u16,
    pub avg_brightness: u8,
    pub max_brightness: u8,
    pub range: u8,
    pub radius: u8,
    pub boundary_left: u8,
    pub boundary_right: u8,
    pub boundary_up: u8,
    pub boundary_down: u8,
    pub aspect_ratio: u8,
    pub vx: u8,
    pub vy: u8,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ObjectReport {
    pub mot_data_nf: [MotData; 16],
    pub mot_data_wf: [MotData; 16],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct CombinedMarkersReport {
    pub nf_points: [Point2<u16>; 16],
    pub wf_points: [Point2<u16>; 16],
    pub nf_radii: [u8; 16],
    pub wf_radii: [u8; 16],
}

#[derive(Clone, Copy, Debug, Default)]
pub struct AccelReport {
    pub accel: [f64; 3],
    pub gyro: [f64; 3],
}

#[derive(Clone, Copy, Debug)]
pub struct StreamUpdate {
    pub mask: u8,
    pub active: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct AimPointReport {
    pub x: i16,
    pub y: i16,
    pub screen_id: u8,
}

#[derive(Clone, Copy, Debug)]
pub enum Error {
    UnexpectedEof { packet_type: Option<PacketType> },
    UnrecognizedPacketId,
    UnrecognizedPort,
    UnrecognizedMarkerPattern,
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error as S;
        match self {
            S::UnexpectedEof { packet_type: None } => write!(f, "unexpected eof"),
            S::UnexpectedEof { packet_type: Some(p) } => write!(f, "unexpected eof, packet id {p:?}"),
            S::UnrecognizedPacketId => write!(f, "unrecognized packet id"),
            S::UnrecognizedPort => write!(f, "unrecognized port"),
            S::UnrecognizedMarkerPattern => write!(f, "unrecognized marker pattern"),
        }
    }
}

impl StdError for Error {}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum Port {
    Nf,
    Wf,
}
impl TryFrom<u8> for Port {
    type Error = Error;
    fn try_from(n: u8) -> Result<Self, Self::Error> {
        match n {
            0 => Ok(Self::Nf),
            1 => Ok(Self::Wf),
            _ => Err(Error::UnrecognizedPort),
        }
    }
}

#[repr(u8)]
#[derive(Copy, Clone, Debug)]
pub enum PacketType {
    WriteRegister, // a.k.a. Poke
    ReadRegister,  // a.k.a. Peek
    ReadRegisterResponse,
    WriteConfig,
    ReadConfig,
    ReadConfigResponse,
    ObjectReportRequest,
    ObjectReport,
    CombinedMarkersReport,
    AccelReport,
    ImpactReport,
    StreamUpdate,
    FlashSettings,
    End,
}

impl TryFrom<u8> for PacketType {
    type Error = Error;
    fn try_from(n: u8) -> Result<Self, Self::Error> {
        match n {
            0 => Ok(Self::WriteRegister),
            1 => Ok(Self::ReadRegister),
            2 => Ok(Self::ReadRegisterResponse),
            3 => Ok(Self::WriteConfig),
            4 => Ok(Self::ReadConfig),
            5 => Ok(Self::ReadConfigResponse),
            6 => Ok(Self::ObjectReportRequest),
            7 => Ok(Self::ObjectReport),
            8 => Ok(Self::CombinedMarkersReport),
            9 => Ok(Self::AccelReport),
            10 => Ok(Self::ImpactReport),
            11 => Ok(Self::StreamUpdate),
            12 => Ok(Self::FlashSettings),
            13 => Ok(Self::End),
            _ => Err(Error::UnrecognizedPacketId),
        }
    }
}

impl Packet {
    pub fn ty(&self) -> PacketType {
        match self.data {
            PacketData::WriteRegister(_) => PacketType::WriteRegister,
            PacketData::ReadRegister(_) => PacketType::ReadRegister,
            PacketData::ReadRegisterResponse(_) => PacketType::ReadRegisterResponse,
            PacketData::WriteConfig(_) => PacketType::WriteConfig,
            PacketData::ReadConfig => PacketType::ReadConfig,
            PacketData::ReadConfigResponse(_) => PacketType::ReadConfigResponse,
            PacketData::ObjectReportRequest(_) => PacketType::ObjectReportRequest,
            PacketData::ObjectReport(_) => PacketType::ObjectReport,
            PacketData::CombinedMarkersReport(_) => PacketType::CombinedMarkersReport,
            PacketData::AccelReport(_) => PacketType::AccelReport,
            PacketData::ImpactReport => PacketType::ImpactReport,
            PacketData::StreamUpdate(_) => PacketType::StreamUpdate,
            PacketData::FlashSettings => PacketType::FlashSettings,
        }
    }

    pub fn parse(bytes: &mut &[u8]) -> Result<Self, Error> {
        let [words1, words2, ty, id, ..] = **bytes else {
            return Err(Error::UnexpectedEof { packet_type: None });
        };

        let words = u16::from_le_bytes([words1, words2]);
        let ty = PacketType::try_from(ty)?;

        let len = usize::from(words)*2;
        if bytes.len() < len {
            return Err(Error::UnexpectedEof { packet_type: Some(ty) });
        }
        *bytes = &bytes[4..];
        let data = match ty {
            PacketType::WriteRegister => PacketData::WriteRegister(WriteRegister::parse(bytes)?),
            PacketType::ReadRegister => PacketData::ReadRegister(Register::parse(bytes, ty)?),
            PacketType::ReadRegisterResponse => PacketData::ReadRegisterResponse(ReadRegisterResponse::parse(bytes)?),
            PacketType::WriteConfig => PacketData::WriteConfig(GeneralConfig::parse(bytes, ty)?),
            PacketType::ReadConfig => PacketData::ReadConfig,
            PacketType::ReadConfigResponse => PacketData::ReadConfigResponse(GeneralConfig::parse(bytes, ty)?),
            PacketType::ObjectReportRequest => PacketData::ObjectReportRequest(ObjectReportRequest{}),
            PacketType::ObjectReport => PacketData::ObjectReport(ObjectReport::parse(bytes)?),
            PacketType::CombinedMarkersReport => PacketData::CombinedMarkersReport(CombinedMarkersReport::parse(bytes)?),
            PacketType::AccelReport => PacketData::AccelReport(AccelReport::parse(bytes)?),
            PacketType::ImpactReport => PacketData::ImpactReport,
            PacketType::StreamUpdate => PacketData::StreamUpdate(StreamUpdate::parse(bytes)?),
            PacketType::FlashSettings => PacketData::FlashSettings,
            p => unimplemented!("{:?}", p),
        };
        Ok(Self { id, data })
    }

    pub fn serialize(&self, buf: &mut Vec<u8>) {
        macro_rules! calculate_length {
            ($ty:ty) => {{
                assert_eq!(std::mem::align_of::<$ty>(), 1);
                std::mem::size_of::<$ty>() as u16
            }};
        }

        let len = match &self.data {
            PacketData::WriteRegister(_) => calculate_length!(WriteRegister),
            PacketData::ReadRegister(_) => calculate_length!(Register)+1,
            PacketData::ReadRegisterResponse(_) => calculate_length!(ReadRegisterResponse)+1,
            PacketData::WriteConfig(_) => 4,
            PacketData::ReadConfig => 0,
            PacketData::ReadConfigResponse(_) => 4,
            PacketData::ObjectReportRequest(_) => calculate_length!(ObjectReportRequest),
            PacketData::ObjectReport(_) => 514,
            PacketData::CombinedMarkersReport(_) => 112,
            PacketData::AccelReport(_) => 16,
            PacketData::ImpactReport => 0,
            PacketData::StreamUpdate(_) => calculate_length!(StreamUpdate),
            PacketData::FlashSettings => 0,
        };
        let words = u16::to_le_bytes((len + 4) / 2);
        let ty = self.ty();
        buf.reserve(4 + usize::from(len));
        buf.extend_from_slice(&[words[0], words[1], ty as u8, self.id]);
        match &self.data {
            PacketData::WriteRegister(x) => x.serialize(buf),
            PacketData::ReadRegister(x) => x.serialize(buf),
            PacketData::ReadRegisterResponse(x) => x.serialize(buf),
            PacketData::WriteConfig(x) => x.serialize(buf),
            PacketData::ReadConfig => (),
            PacketData::ReadConfigResponse(x) => x.serialize(buf),
            PacketData::ObjectReportRequest(_) => (),
            PacketData::ObjectReport(x) => x.serialize(buf),
            PacketData::CombinedMarkersReport(x) => x.serialize(buf),
            PacketData::AccelReport(x) => unimplemented!(),
            PacketData::ImpactReport => (),
            PacketData::StreamUpdate(x) => buf.extend_from_slice(&[x.mask as u8, x.active as u8]),
            PacketData::FlashSettings => (),
        }
    }
}

impl PacketData {
    pub fn read_register_response(self) -> Option<ReadRegisterResponse> {
        match self {
            PacketData::ReadRegisterResponse(x) => Some(x),
            _ => None,
        }
    }

    pub fn read_config_response(self) -> Option<GeneralConfig> {
        match self {
            PacketData::ReadConfigResponse(x) => Some(x),
            _ => None,
        }
    }

    pub fn object_report(self) -> Option<ObjectReport> {
        match self {
            PacketData::ObjectReport(x) => Some(x),
            _ => None,
        }
    }

    pub fn combined_markers_report(self) -> Option<CombinedMarkersReport> {
        match self {
            PacketData::CombinedMarkersReport(x) => Some(x),
            _ => None,
        }
    }

    pub fn accel_report(self) -> Option<AccelReport> {
        match self {
            PacketData::AccelReport(x) => Some(x),
            _ => None,
        }
    }
}

impl Register {
    pub fn parse(bytes: &mut &[u8], pkt_ty: PacketType) -> Result<Self, Error> {
        use Error as E;
        let [port, bank, address, _, ..] = **bytes else {
            return Err(E::UnexpectedEof { packet_type: Some(pkt_ty) });
        };
        let port = port.try_into()?;
        *bytes = &bytes[4..];
        Ok(Self { port, bank, address })
    }

    pub fn serialize(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&[self.port as u8, self.bank, self.address, 0]);
    }
}

impl ReadRegisterResponse {
    pub fn parse(bytes: &mut &[u8]) -> Result<Self, Error> {
        use Error as E;
        let [bank, address, data, _, ..] = **bytes else {
            return Err(E::UnexpectedEof { packet_type: Some(PacketType::ReadRegisterResponse) });
        };
        *bytes = &bytes[4..];
        Ok(Self { bank, address, data })
    }

    pub fn serialize(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&[self.bank, self.address, self.data, 0]);
    }
}

impl WriteRegister {
    pub fn parse(bytes: &mut &[u8]) -> Result<Self, Error> {
        use Error as E;
        let [port, bank, address, data, ..] = **bytes else {
            return Err(E::UnexpectedEof { packet_type: Some(PacketType::WriteRegister) });
        };
        let port = port.try_into()?;
        *bytes = &bytes[4..];
        Ok(Self { port, bank, address, data })
    }

    pub fn serialize(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&[self.port as u8, self.bank, self.address, self.data]);
    }
}

impl GeneralConfig {
    pub fn parse(bytes: &mut &[u8], pkt_ty: PacketType) -> Result<Self, Error> {
        use Error as E;
        let [impact_threshold, accel_odr0, accel_odr1, _, ..] = **bytes else {
            return Err(E::UnexpectedEof { packet_type: Some(pkt_ty) });
        };
        let accel_odr: [u8; 2] = [accel_odr0, accel_odr1];
        let accel_odr = u16::from_le_bytes(accel_odr);
        *bytes = &bytes[4..];
        Ok(Self { impact_threshold, accel_odr })
    }

    pub fn serialize(&self, buf: &mut Vec<u8>) {
        let accel_odr: [u8; 2] = u16::to_le_bytes(self.accel_odr);
        buf.extend_from_slice(&[self.impact_threshold, accel_odr[0], accel_odr[1], 0]);
    }
}

impl MotData {
    pub fn parse(bytes: &mut &[u8]) -> Result<Self, Error> {
        let mot_data = MotData {
            area: bytes[0] as u16 | ((bytes[1] as u16) << 8),
            cx: bytes[2] as u16 | ((bytes[3] & 0x0f) as u16) << 8,
            cy: bytes[4] as u16 | ((bytes[5] & 0x0f) as u16) << 8,
            avg_brightness: bytes[6],
            max_brightness: bytes[7],
            radius: bytes[8] & 0x0f,
            range: bytes[8] >> 4,
            boundary_left: bytes[9] & 0x7f,
            boundary_right: bytes[10] & 0x7f,
            boundary_up: bytes[11] & 0x7f,
            boundary_down: bytes[12] & 0x7f,
            aspect_ratio: bytes[13],
            vx: bytes[14],
            vy: bytes[15],
        };
        *bytes = &bytes[16..];
        Ok(mot_data)
    }

    pub fn serialize(&self, buf: &mut Vec<u8>) {
        buf.extend_from_slice(&[
            self.area as u8,
            (self.area >> 8) as u8,
            self.cx as u8,
            (self.cx >> 8) as u8,
            self.cy as u8,
            (self.cy >> 8) as u8,
            self.avg_brightness,
            self.max_brightness,
            self.radius | self.range << 4,
            self.boundary_left,
            self.boundary_right,
            self.boundary_up,
            self.boundary_down,
            self.aspect_ratio,
            self.vx,
            self.vy,
        ]);
    }
}

impl ObjectReport {
    pub fn parse(bytes: &mut &[u8]) -> Result<Self, Error> {
        use Error as E;
        let data = &mut &bytes[..512];
        *bytes = &bytes[512..];
        let [_format, _, ..] = **bytes else {
            return Err(E::UnexpectedEof { packet_type: Some(PacketType::ObjectReport) });
        };
        *bytes = &bytes[2..];
        Ok(Self {
            mot_data_nf: [(); 16].map(|_| MotData::parse(data).expect("MotData parse error")),
            mot_data_wf: [(); 16].map(|_| MotData::parse(data).expect("MotData parse error")),
        })
    }

    pub fn serialize(&self, buf: &mut Vec<u8>) {
        for i in 0..16 {
            self.mot_data_nf[i].serialize(buf);
        }
        for i in 0..16 {
            self.mot_data_wf[i].serialize(buf);
        }
        buf.extend_from_slice(&[1, 0]);
    }
}

impl CombinedMarkersReport {
    pub fn parse(bytes: &mut &[u8]) -> Result<Self, Error> {
        use Error as E;
        if bytes.len() < 112 {
            return Err(E::UnexpectedEof { packet_type: Some(PacketType::CombinedMarkersReport) });
        }

        let data = &mut &bytes[..112];
        *bytes = &bytes[112..];
        let mut positions = [Point2::new(0, 0); 16*2];
        for i in 0..positions.len() {
            // x, y is 12 bits each
            let x = u16::from_le_bytes([data[0], data[1] & 0x0f]);
            let y = (data[1] >> 4) as u16 | ((data[2] as u16) << 4);
            positions[i] = Point2::new(x, y);
            *data = &data[3..];
        }
        let nf_positions = positions[..16].try_into().unwrap();
        let wf_positions = positions[16..].try_into().unwrap();

        // lower nibble and upper nibble are both radiuses, so we need both as separate elements in the final array
        let mut radii = [0; 32];
        let mut i = 0;
        for _ in 0..16 {
            let r = data[0] & 0x0f;
            radii[i] = r;
            let r = data[0] >> 4;
            radii[i+1] = r;
            i += 2;
            *data = &data[1..];
        }

        let nf_radii = radii[..16].try_into().unwrap();
        let wf_radii = radii[16..].try_into().unwrap();

        Ok(Self { nf_points: nf_positions, wf_points: wf_positions, nf_radii, wf_radii })
    }

    pub fn serialize(&self, buf: &mut Vec<u8>) {
        let before = buf.len();
        for p in self.nf_points.iter().chain(&self.wf_points) {
            let XY { x, y } = **p;
            let byte0 = x & 0xff;
            let byte1 = ((x >> 8) & 0x0f) | ((y & 0x0f) << 4);
            let byte2 = y >> 4;
            buf.extend_from_slice(&[byte0 as u8, byte1 as u8, byte2 as u8]);
        }

        for w in self.nf_radii.chunks(2).chain(self.wf_radii.chunks(2)) {
            buf.push((w[0] & 0x0f) | (w[1] << 4));
        }
        let after = buf.len();
    }
}

impl AccelReport {
    pub fn parse(bytes: &mut &[u8]) -> Result<Self, Error> {
        // accel: x, y, z, 16384 = 1g
        // gyro: x, y, z, 16.4 = 1dps
        let data = &mut &bytes[..12];
        let accel = [(); 3].map(|_| {
            let x = i16::from_le_bytes([data[0], data[1]]);
            let x = x as f64 / 16384.0;
            *data = &data[2..];
            x
        });
        let gyro = [(); 3].map(|_| {
            let x = i16::from_le_bytes([data[0], data[1]]);
            let x = x as f64 / 16.4;
            *data = &data[2..];
            x
        });
        *bytes = &bytes[12..];
        Ok(Self { accel, gyro })
    }
}

impl StreamUpdate {
    pub fn parse(bytes: &mut &[u8]) -> Result<Self, Error> {
        let stream_update = StreamUpdate {
            mask: bytes[0],
            active: bytes[1] != 0,
        };
        *bytes = &bytes[2..];
        Ok(stream_update)
    }
}
