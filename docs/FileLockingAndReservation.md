# File RW locking

Each Storage Node maintains its own locking table, storing locks for the file chunks present on it. When performing any
operation on the file contents (read, write) a lock must be used.  
Only one node can hold the write lock concurrently, as it is writing to the chunk.
Multiple nodes can hold a read lock concurrently, as they read the chunk.
Lastly, only one type of lock, either one write lock, or many read locks, can be held concurrently.

This ensures that only one node can write at a time, but many nodes can read concurrently.

When obtaining the lock, a node sends out a lock request to every node holding the file chunks it wants to access
(including itself, if it itself is storing a file chunk). If a lock is immediately available, the locker returns the
lock during the initial locking request. Otherwise, it returns a "please wait" response, and only once the lock is
available, sends back the lock. Each lock is automatically released once the node holding the lock finishes performing
its operation. This process is done by the locker node autonomously.

The locker node should only accept requests to the file chunk from a node holding the appropriate lock on it.

Every locking operation has an applicable timeout.

# File reservations

A reservation must be made before writing new data to any node. A reservation is made via an internal request to the
destination node.
Such a request must contain the size of space that needs to be reserved. If successful, the destination node returns a
newly generated chunk ID. Otherwise, it returns an error and the origin node precedes to try other nodes with the
[required amount of disk space](#keeping-the-nodes-up-to-date). Once a reservation is made, the original node is
automatically granted a write lock.

When reserving space on an already existing chunk id, when overwriting a file, the write lock must first be separately
obtained.

# Keeping the nodes up to date

Each node periodically refreshes the controller server with its available storage. This info is then distributed to
every node, so that they know which nodes can have their space reserved.