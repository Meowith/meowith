<img src="assets/meowith-full.svg" height="100" alt="Meowith logo">

Meowith is a scalable, distributed blob storage solution built in Rust, designed for performance and reliability. It
features a modular architecture with dedicated nodes for management, file access, and orchestration, and includes a
built-in protocol framework for customizable communication.

## Table of Contents

- [Introduction](#introduction)
- [Installation](#installation)
- [Usage](#usage)
- [Feature set](#feature-set)
- [License](#license)

## Introduction
This system is a learning project, aiming to create a distributed file system solution. It features a modular architecture with distinct nodes for management, file access, and orchestration. Key aspects include:

- **Modular Design:** Separate nodes handle management, file access, and system orchestration for efficient performance.
- **Distributed Storage:** Data is spread across multiple nodes to ensure high availability and fault tolerance.
- **Sharding:** Supports data sharding for improved scalability and balanced load distribution.
- **Rust Implementation:** Leverages Rust's safety and performance features for low-latency operations and reliability.

## Installation

1. Clone the repository:
   ```bash
   git clone https://github.com/Meowith/meowith
   cd meowith
   ```
2. Build the project:
   ```bash
   cargo build --release --workspace
   ```
3. Distribute the binaries.
   The binaries are in the target directory (`target/`)
4. Set up the controller.
5. Host the frontend  
   This can be done using any web-server you'd like.
6. Generate register codes and register dashboard(s) and node(s)  
   A node instance is created and provided with:
    - The CA file to validate SSL connections
    - The valid node register code, via the `REGISTER_CODE` env var
    - The socket address of the controller /internal web '

   Upon starting with the `REGISTER_CODE` env var the node performs a request to the controller
   server with the register code and its node type

## Usage

Meowith management is done through the frontend and the dashboard service.
Accessing and interacting with the file system
is done via the Meowith nodes.

Connecting to the file system can be done via either the Meowith-cli, or any of the available connectors.
As an alternative, it can be done vie the http api directly.

## Feature set

- [Bucket system](#Buckets)
    - [Fs support](#Files)
        - Quota
- [App System](#Applications)
    - [Users](#users)
    - [Roles](#roles)
    - [Quotas](#application-quotas)
    - Management Panel

## Buckets

Buckets contain a flat list of file entries identified by their unique name.
While the list is flat, it can still be queried using directory paths.
Furthermore, directories themselves can be created as well.

The Bucket structure is as follows:

```
Your Bucket:
├ file1
├ file2
├ folder/file1
└ folder/file2
```

## Files

Each file entry contains additional metadata about itself:

| Property         | Description                                              |
|------------------|----------------------------------------------------------|
| Name             | The unique name of the file, [see more](#file-names)     |
| Size             | The size of the file                                     |
| Creation date    | The date of the original creation of the file            |
| Last Modify Date | The date of the last file content modification           |
| Directory        | The id of the parent directory, all 0's for the root dir |

### File names

We allow any Unicode string as a file name up to a length of 2048 characters.
Note that the file name includes its full folder path.

# Applications

The Meowith Application is a top-level data organization unit, containing [Buckets](#buckets), which hold the actual
data, and [members](#users) that can access the data in a way specified by their permissions.

## Users

Each user can be an owner of many applications, as well as be a member of other applications.
Account permissions are determined by the role(s) assigned to the account.

## Roles

An account role is a template containing permissions which will be granted to the user owning the role.
All permissions require one or more scopes, specifying either:
1. the buckets on which the permission is applicable
2. or that the permission is an app permission instead.

Bucket permissions:

| Name            | Description                                                          |
|-----------------|----------------------------------------------------------------------|
| Read            | Read the contents of a file                                          |
| Write           | Create and write to a new file                                       |
| Overwrite       | Overwrite an existing file                                           |
| ListDirectory   | List the contents of a given directory                               |
| ListBucket      | List the contents of the entire bucket at once                       |
| Rename          | Rename an entity                                                     |
| Delete          | Delete an entity                                                     |
| FetchBucketInfo | Fetch information about the bucket, such as its quota or file count. |

App permissions:

| Name            | Description                          |
|-----------------|--------------------------------------|
| CreateBucket    | Create a bucket                      |
| DeleteBucket    | Delete an empty bucket               |
| ListAllTokens   | List tokens created by all app users |
| DeleteAllTokens | Delete the tokens of other users     |
| ManageRoles     | Manage user roles and permissions    |

## Application Quotas

Each application has its summary storage quota.
The combined quota of all the buckets owned by the app must not exceed the Summary Application Quota

## License

Meowith
Copyright &copy; 2025    Michal-python & KsanStone

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <http://www.gnu.org/licenses/>.