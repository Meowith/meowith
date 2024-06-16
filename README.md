# Meowith

A horizontally scalable, distributed blob storage solution.

## Feature set

- [Bucket system](#Buckets)
    - [Fs support](#Files)
        - Quota
- [App System](#Applications)
    - [Users](#users)
    - [Roles](#roles)
    - [Quotas](#application-quotas)
    - Management Panel
- [Sharding](#sharding)

## To Consider

- encryption

# Buckets

Buckets contain a flat list of file entries identified by their unique name.
While the list is flat it can still be queried using folder paths.

The Bucket structure is as follows:

```
Your Bucket:
├ file1
├ file2
├ folder/file1
└ folder/file2
```

Each Bucket contains its separate set of properties

| Option     | Description                                                  |
|------------|--------------------------------------------------------------|
| Name       | The unique name of the Bucket                                |
| Encryption | Whether to encrypt file contents on disk or not              |
| Disk Quota | The Disk Space limit for the files stored inside this bucket |

## Files

Each file entry contains additional metadata about itself:

| Property         | Description                                          |
|------------------|------------------------------------------------------|
| Name             | The unique name of the file, [see more](#file-names) |
| Size             | The size of the file                                 |
| Creation date    | The date of the original creation of the file        |
| Last Modify Date | The date of the last file content modification       |

## File names

We allow any unicode string as a file name up to a length of 2048 characters. Note that the file name includes its full folder path.

# Applications

The Meowith Application is a top-level data organization unit, containing [Buckets](#buckets), which hold the actual data,
and [Users](#users) that can access the data in a way specified by their permissions.

## Users

Each Application has its own set of users, who are able to access specified resources within it.
By Default One Administrative account with the provided credentials is created, this account can manage the entire application and as such, access to it should be limited.
This account cannot be deleted to prevent accidental lock-outs.

Account permissions are determined by the role(s) assigned to the account.

## Roles

An account role is a template containing permissions which will be granted to the user owning the role.
Most permissions require a permission scope, specifying the buckets on which the permission is applicable.

The permission scope is a string consisting of a bucket name and/or glob-like wildcards.
For example:

The scope `bucket1` -  Will match only the bucket named "bucket1"   
`bucket*` -  Will match any bucket starting with "bucket"

The available permission types are as follows:

| Name      | Description                                                                                                          |
|-----------|----------------------------------------------------------------------------------------------------------------------|
| Read      | Permits read operations on the provided scope                                                                        |
| ReadWrite | Permits read and write operations on the provided scope                                                              |
| Admin     | Allows for the administration over the entire application. Ex. creating/modifying buckets, R/W access to all Buckets |

## Application Quotas

Each application has its summary storage quota.
The combined quota of all the buckets owned by the app must not exceed the Summary Application Quota

# Sharding

## Node creation

1. A node worker is started up with the generated register code. (And any required ssl certificates if necessary)
2. The worker registers with the CnC server using the code.
3. The worker receives a renewal token, and all the necessary configuration.
4. Using the renewal token generates an access token.

## Connection Security

CnC server generates a ssl certificate for each node based on its domain configuration in the database then the server distributes the certificate to desired node as well as a CA file so that all nodes can verify it.

## Architecture
![](https://ksancloud.pl:5000/api/file/download/public/najd(1).png)