# Testing Instructions

## Test the complete pipeline:

### Terminal 1 - Start Receiver:
```bash
./target/release/srt-receiver --bind 127.0.0.1 --listen 20000 --output udp://127.0.0.1:20002 --stats 1
```

### Terminal 2 - Start Sender (with ffmpeg):
```bash
ffmpeg -re -f lavfi -i testsrc=size=1280x720:rate=30 -c:v libx264 -preset ultrafast -tune zerolatency -f mpegts - | ./target/release/srt-sender --input - --path 127.0.0.1:20000 --stats 1
```

## What to look for in the logs:

### Expected Sender Logs:
1. "Initiating handshake with 127.0.0.1:20000..."
2. "Received X bytes in handshake loop from 127.0.0.1:20000"
3. "Handshake successful with 127.0.0.1:20000, remote_socket_id=Some(999)"
4. "Sending first data packet: seq=X, dest_socket_id=999, size=Y"
5. Regular stats: "Sent X packets, Y.YY Mbps"

### Expected Receiver Logs:
1. "Listening on: 127.0.0.1:20000"
2. "Received handshake request from 127.0.0.1:XXXXX, sender_socket_id=Y"
3. "Created connection for member 1, remote_socket_id=Some(Y)"
4. "Sent X bytes of handshake agreement to 127.0.0.1:XXXXX"
5. "Received first data packet: seq=X, dest_socket_id=999, size=Y"
6. Regular stats: "Stats: 1 members, buffered=X, ready=Y"

## Key diagnostic points:

### If handshake fails:
- Check if sender shows "Handshake successful" with remote_socket_id=Some(999)
- If not, handshake exchange is broken

### If data packets not received:
- Check sender logs for "Sending first data packet" with dest_socket_id=999
- Check receiver logs for "Received first data packet"
- If sender shows dest_socket_id=0, handshake processing failed

### If buffered=0 in receiver stats:
- Check receiver logs for "Error processing data packet"
- This means packets are received but bonding layer is rejecting them

