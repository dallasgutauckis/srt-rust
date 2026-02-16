use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use srt_protocol::packet::{ControlPacket, ControlType, DataPacket, MsgNumber, Packet};
use srt_protocol::sequence::SeqNumber;

fn bench_data_packet_serialize(c: &mut Criterion) {
    let seq = SeqNumber::new(1000);
    let msg = MsgNumber::new(100);
    let payload = Bytes::from(vec![0u8; 1316]); // Typical payload size

    let packet = DataPacket::new(seq, msg, 5000, 9999, payload);

    c.bench_function("data_packet_serialize", |b| {
        b.iter(|| {
            let bytes = black_box(&packet).to_bytes();
            black_box(bytes);
        });
    });
}

fn bench_data_packet_deserialize(c: &mut Criterion) {
    let seq = SeqNumber::new(1000);
    let msg = MsgNumber::new(100);
    let payload = Bytes::from(vec![0u8; 1316]);

    let packet = DataPacket::new(seq, msg, 5000, 9999, payload);
    let bytes = packet.to_bytes();

    c.bench_function("data_packet_deserialize", |b| {
        b.iter(|| {
            let packet = DataPacket::from_bytes(black_box(&bytes)).unwrap();
            black_box(packet);
        });
    });
}

fn bench_control_packet_serialize(c: &mut Criterion) {
    let control_info = Bytes::from(vec![0u8; 100]);
    let packet = ControlPacket::new(ControlType::Ack, 0x1234, 5000, 10000, 9999, control_info);

    c.bench_function("control_packet_serialize", |b| {
        b.iter(|| {
            let bytes = black_box(&packet).to_bytes();
            black_box(bytes);
        });
    });
}

fn bench_seq_number_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("sequence_number");

    group.bench_function("increment", |b| {
        let mut seq = SeqNumber::new(1000);
        b.iter(|| {
            seq.increment();
            black_box(&seq);
        });
    });

    group.bench_function("distance", |b| {
        let a = SeqNumber::new(1000);
        let b = SeqNumber::new(2000);
        b.iter(|| {
            let dist = black_box(a).distance_to(black_box(b));
            black_box(dist);
        });
    });

    group.bench_function("comparison", |b| {
        let a = SeqNumber::new(1000);
        let b = SeqNumber::new(2000);
        b.iter(|| {
            let result = black_box(a).lt(black_box(b));
            black_box(result);
        });
    });

    group.finish();
}

fn bench_msg_number_encode_decode(c: &mut Criterion) {
    let msg = MsgNumber::new(12345);

    c.bench_function("msg_number_encode", |b| {
        b.iter(|| {
            let raw = black_box(msg).to_raw();
            black_box(raw);
        });
    });

    c.bench_function("msg_number_decode", |b| {
        let raw = msg.to_raw();
        b.iter(|| {
            let decoded = MsgNumber::from_raw(black_box(raw));
            black_box(decoded);
        });
    });
}

criterion_group!(
    benches,
    bench_data_packet_serialize,
    bench_data_packet_deserialize,
    bench_control_packet_serialize,
    bench_seq_number_ops,
    bench_msg_number_encode_decode
);
criterion_main!(benches);
