# Controller

1. Controls all the nodes *(Via internal webserver)*
   - Autoconfigures SSL certs
   - Monitors their health
   - Discovery service
2. Provides admin access to the operator *(Via public webserver)*
   - Global panel (including logs, health status) (via the web-ui)

# Storage Node

1. Stores file chunks on the node's physical drive(s) *(Via internal webserver)*
2. Provides access to files and directories for end users *(Via public webserver)*
   - Listing folders
   - Listing buckets
   - File CRUD operations.

# WebFront

1. Provides API access for end users to the user panel (via the web-ui)
   - App CRUD operations.
   - App member management.
   - Bucket CRUD operations.