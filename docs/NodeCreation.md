# NodeCreation

Node creation is performed using the node register code in the following fashion:

1. A node instance is created and provided with:
    - The CA file to validate SSL connections
    - The valid node register code, via the `REGISTER_CODE` env var
    - The socket address of the controller /internal web server
2. Upon starting with the `--register-code <code>` commandline argument the node performs a request to the controller
   server with the register code and its node type
3. If the code is valid, the controller server invalidates the code, registers the node, records its address and type,
   and returns it the refresh token
4. Using the refresh token, the node gets an access token from the controller.
5. Using the access token, the node provides to the controller other metadata about it (ex. max storage)
6. Lastly, the node gets its SSL certificates from the controller.