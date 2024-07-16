# File Transfer Authorization

Every file transfer request is authorized by a JWT (meta information of that token is stored in db)

# JWT Structure
```json5
{
  "app_id": "", //Application id for app identification
  "issuer_id": "", //ID of the user that issued creation of the token 
  "nonce": "", //Value that is used for token validation (Stored in database)
  "perms": [ //List of allowed buckets with sufficient permissions assigned accordingly
    {
      "scope": "", //bucket name regarding current app 
      "allowance": 0 //encoded permissions num that indicates which actions can be performed using this token on the bucket 
    }
    // ...
  ]
}
```

# App management

Owner has authority over every created access token in the panel of an app.
Owner of the app (or just owner of the token) can invalidate and/or edit tokens:
- **Invalidation** - 
Simply deletes the record from the database so the token can no longer be used to perform actions  
- **Edit** - 
Just changes the nonce in the database and generates new token with correct nonce 


# User roles

Permissions that are linked with user by permission container,
also known as user roles, are just a limit for the user, that creates the token,
for how high permissions he can create the token with.