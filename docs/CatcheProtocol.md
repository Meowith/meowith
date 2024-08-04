Catche Protocol

Protocol mainly used for internode cache invalidation

# Packet format

Each packet begins with the universal header:

| Cache ID | Cache Key Size | Cache key       |
|----------|----------------|-----------------|
| 4 Bytes  | 4 Bytes        | \[Payload Size] |

