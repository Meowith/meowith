# MGPP

Meowith General Purpose Protocol

Protocol that utilizes controller -> nodes & dashboards communication

## Packet format

Each packet begins with the universal header (derived from protocol framework):

| Packet type | Payload length | Content         |
|-------------|----------------|-----------------|
| 1 Byte      | 4 Bytes        | \[Payload size] |


## Packets

- **Invalidate cache** 
  Packet for invalidating internode cache (legacy Catche packet) (Packet ID: 0x01)

| Cache ID | Cache Key size | Cache key |
|----------|----------------|-----------|
| 4 Bytes  | 4 Bytes        | 1..2^16-8 |
