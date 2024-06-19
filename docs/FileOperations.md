# File Operations

Each operation needs to be authorized.

## Read

1. A read request goes to any storage node.
2. The node fetched the file metadata from the database. Then, it gets a read lock on it.
3. If the file is found, the node streams it to the end user, by fetching chunks from all the nodes containing the
   file.

## Upload

File upload is split into two categories. Before each, a write lock must be obtained on the file.
If possible, the node to which the request by the end user is made should reserve its own disk space, to avoid internode
traffic.

- **Durable**, *preferred*\* for large files (> 50MiB)
    1. A POST request to the node is made, with the file name, bucket and size, initiating the upload.
    2. After getting the write lock on the file, and reserving space, the server respons with an upload token.
    3. Using the token, the user uploads the file using an HTTP PUT request.

  If the upload fails for any reason, the user may re-initiate it using the upload token, within an hour of the
  interruption. After the hour has passed, the corrupted file is to be deleted.

    1. A POST request to the node is made with the file name, bucket, size and the upload token, re-initiating the
       upload.
    2. After validation, the server respons with the bytes of the file already written to the service.
    3. Using the token, the user uploads the remainder of the file using an HTTP PUT request.

- **Non-Durable**, *preferred*\* for smaller files (<= 50MiB)
    1. A POST request to the node is made, with the file name, bucket and size, as well as the file itself.
    2. After getting the write lock on the file, and reserving space, the server writes the file.

  If the upload fails for any reason, the user may re-initiate it using the upload token, within an hour of the
  interruption. After the hour has passed, the corrupted file is to be deleted.

. * preferred, as this rule is not enforced.

## Overwrite

Works the same way as upload. Requires additional permissions.

## Rename

Done using an HTTP POST, the server must get a read lock on the old file name, as well as the new file name.

## Delete

Done using an HTTP DELETE, the server must get a read lock on the old file.