use ats_usb::device::{decode_slip_frame, SLIP_FRAME_END};
use ats_usb::packets::vm::{Packet, PacketData, Port, Register};
use std::net::{Ipv4Addr, SocketAddr};
// use multicast_socket::MulticastSocket;
use socket2::{Domain, Protocol, SockAddr, Socket, Type};
use std::net::UdpSocket;

fn main() {
    let local_addr = SockAddr::from(SocketAddr::new(Ipv4Addr::UNSPECIFIED.into(), 0));

    // todo MulticastSocket has to be turned into Ext for std, socket2, and tokio
    // let client = MulticastSocket::all_interfaces(multicast_addr_std).unwrap();

    let client =
        Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).expect("ipv4 dgram socket");

    client.set_broadcast(true).unwrap();

    client.bind(&local_addr).unwrap();

    let broadcast_addr = SockAddr::from(SocketAddr::new(
        std::net::IpAddr::V4(Ipv4Addr::BROADCAST),
        23456,
    ));
    client.send_to(&[255, 1], &broadcast_addr).unwrap();

    let mut udp_packet = vec![1, 0, 255];
    let pkt = Packet {
        id: 0,
        data: PacketData::ReadRegister(Register {
            port: Port::Wf,
            bank: 0x00,
            address: 0x02,
        }),
    };
    pkt.serialize(&mut udp_packet);
    client.send_to(&udp_packet, &broadcast_addr).unwrap();

    // let mut udp_packet = vec![1, 0, 255];
    // let pkt = Packet {
    //     id: 1,
    //     data: PacketData::StreamUpdate(StreamUpdate { mask: 0b0100 | 0b0010, active: true })
    // };
    // pkt.serialize(&mut udp_packet);
    // client.send_to(&udp_packet, &multicast_addr).unwrap();

    let client: UdpSocket = client.into();

    let mut data = vec![0; 1472];
    let mut slip_buf = vec![];
    let mut total_accel_samples = 0;
    let start_time = std::time::Instant::now();
    let mut total_combined_marker_samples = 0;

    fn process_one(
        start_time: std::time::Instant,
        data: &mut Vec<u8>,
        _total_accel_samples: &mut u64,
        total_combined_marker_samples: &mut u64,
        addr: SocketAddr,
    ) {
        match Packet::parse(&mut data.as_slice()) {
            Ok(pkt) => {
                println!("Received {:?} from {addr}", pkt);
                match pkt.data {
                    PacketData::CombinedMarkersReport(_) => {
                        let elapsed = start_time.elapsed().as_secs_f64();
                        *total_combined_marker_samples += 1;
                        let combined_marker_hz_avg =
                            1. / (elapsed / *total_combined_marker_samples as f64);
                        println!("Combined Marker: {:?} Hz", combined_marker_hz_avg);
                    }
                    _ => {}
                }
            }
            Err(e) => match e {
                ats_usb::packets::vm::Error::UnexpectedEof { .. } => {}
                ats_usb::packets::vm::Error::UnrecognizedPacketId(_) => {}
                ats_usb::packets::vm::Error::UnrecognizedPort => {}
                _ => {
                    println!("Error: {:?}", e);
                }
            },
        }
    }

    loop {
        let resp = client.recv_from(&mut data);
        if let Ok((len, addr)) = resp {
            // if let Ok(msg) = client.receive() {
            // data = msg.data;
            let data = &data[..len];
            println!("Received {:?} from {addr}", &data);
            // let addr = msg.origin_address;
            // println!("Received {:?} from {addr}", &data);
            if data[1] != 0 || data[0] == 255 {
                continue;
            }

            // check if &data[2..len] contains end of slip frame
            let data = &data[2..len];

            slip_buf.extend_from_slice(&data);
            while slip_buf.contains(&SLIP_FRAME_END) {
                let end_idx = slip_buf.iter().position(|&x| x == SLIP_FRAME_END).unwrap();
                let mut slice_vec = &mut slip_buf[..end_idx].to_vec();
                if let Ok(()) = decode_slip_frame(slice_vec) {
                    process_one(
                        start_time,
                        &mut slice_vec,
                        &mut total_accel_samples,
                        &mut total_combined_marker_samples,
                        addr,
                    );
                    slip_buf.drain(0..end_idx + 1);
                } else {
                    slip_buf.drain(0..end_idx + 1);
                }
            }
        }
    }
}
