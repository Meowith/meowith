# MDSFTP Protocol
Meowith Distributed Services File Transfer Protocol

# Packet format

Each packet begins with the universal header:

| Packet ID | Stream ID | Payload Size | Content         |
|-----------|-----------|--------------|-----------------|
| 1 Byte    | 4 Bytes   | 2 Bytes      | \[Payload Size] |

All the packet data is stored in the content field

# Packets

## Chunk

- **File upload/download** (`file_chunk` Packet ID: 0x01)

| Flags  | Chunk ID | content   |
|--------|----------|-----------|
| 1 Byte | 4 Bytes  | 1..2^16-2 |

Flags: `is_last` (true/false)

- **Retrieve** (`retreive` Packet ID: 0x02)

| Chunk ID |
|----------|
| 36 Bytes |

- **Put** (`put` Packet ID: 0x03)

| Chunk ID | Chunk Size |
|----------|------------|
| 36 Bytes | 8 Bytes    |

## Reserve

- **Reserve** (`reserve` Packet ID: 0x04)

| Desired Size |
|--------------|
| 8 Bytes      |

- **Reserve success** (`reserve_ok` Packet ID: 0x05)

| Chunk ID |
|----------|
| 36 Bytes |

- **Reserve error** (`reserve_err` Packet ID: 0x06)

| Max space |
|-----------|
| 8 Bytes   |

## Locks

- **Lock request** (`lock_req` Packet ID: 0x07)

| Flags   | Chunk ID  |
|---------|-----------|
| 1 Bytes | 36 Bytes  |

Flags: `kind` (read/write)

- **Lock acquire** (`lock_acquire` Packet ID: 0x08)

| Flags   | Chunk ID  |
|---------|-----------|
| 1 Bytes | 36 Bytes  |

Flags: `kind` (read/write)

- **Lock Error** (`lock_err` Packet ID: 0x09)

| Flags   | Chunk ID  |
|---------|-----------|
| 1 Bytes | 36 Bytes  |

Flags: `kind` (read/write)

## Channels

- **Channel open** (`channel_open` Packet ID: 0x80)

| Channel ID |
|------------|
| 4 Bytes    |

- **Channel close** (`channel_close` Packet ID: 0x81)

| Channel ID |
|------------|
| 4 Bytes    |

- **Channel open error** (`channel_err` Packet ID: 0x82)

| Channel ID |
|------------|
| 4 Bytes    |