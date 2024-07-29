# MDSFTP Protocol

Meowith Distributed Services File Transfer Protocol

# Packet format

Each packet begins with the universal header:

| Packet ID | Stream ID | Payload Size | Content         |
|-----------|-----------|--------------|-----------------|
| 1 Byte    | 4 Bytes   | 4 Bytes      | \[Payload Size] |

All the packet data is stored in the content field

# Packets

## Chunk

- **File upload/download** (`file_chunk` Packet ID: 0x01)

| Flags  | Chunk ID | content   |
|--------|----------|-----------|
| 1 Byte | 4 Bytes  | 1..2^16-5 |

Flags: `is_last` (true/false)

- **Retrieve** (`retreive` Packet ID: 0x02)

| Chunk ID | Chunk Buffer |
|----------|--------------|
| 16 Bytes | 2 Bytes      |

- **Put** (`put` Packet ID: 0x03)

| Flags  | Chunk ID | Chunk Size |
|--------|----------|------------|
| 1 Byte | 16 Bytes | 8 Bytes    |`

- **PutOk** (`put_ok` Packet ID: 0x0D)

| Chunk Buffer |
|--------------|
| 2 Bytes      |

- **PutErr** (`put_err` Packet ID: 0x0E)

| Flags   |
|---------|
| 1 Bytes |

Flags: `error_kind` (not_found/internal)

- **RecvAck** (`receive_ack`, Packet ID: 0x04)

| Chunk ID |
|----------|
| 4 Bytes  |

- **Delete** (`delete_chunk`, Packet ID: 0x05)

| Chunk ID |
|----------|
| 16 Bytes |

## Reserve

- **Reserve** (`reserve` Packet ID: 0x06)

| Flags  | Desired Size |
|--------|--------------|
| 1 Byte | 8 Bytes      |

Flags: `Auto-start` (yes/no), `Durable` (yes\no), `Overwrite` (yes/no)

- **Reserve Cancel** (`reserve_cancel` Packet ID: 0x07)

| Chunk ID |
|----------|
| 16 Bytes |

- **Reserve success** (`reserve_ok` Packet ID: 0x08)

| Chunk ID | Chunk Buffer |
|----------|--------------|
| 16 Bytes | 2 Bytes      |

- **Reserve error** (`reserve_err` Packet ID: 0x09)

| Max space |
|-----------|
| 8 Bytes   |

## Locks

- **Lock request** (`lock_req` Packet ID: 0x0A)

| Flags   | Chunk ID |
|---------|----------|
| 1 Bytes | 16 Bytes |

Flags: `kind` (read/write)

- **Lock acquire** (`lock_acquire` Packet ID: 0x0B)

| Flags   | Chunk ID |
|---------|----------|
| 1 Bytes | 16 Bytes |

Flags: `kind` (read/write)

- **Lock Error** (`lock_err` Packet ID: 0x0C)

| Flags   | Chunk ID |
|---------|----------|
| 1 Bytes | 16 Bytes |

Flags: `lock_kind` (read/write) `error_kind` (not_found/internal)

## Channels

- **Channel open** (`channel_open` Packet ID: 0x80)

- **Channel close** (`channel_close` Packet ID: 0x81)

- **Channel open error** (`channel_err` Packet ID: 0x82)