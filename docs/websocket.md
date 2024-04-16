# Wings<->Panel websocket API

Websockets messages all follow the following JSON schema:

```json
{
	"event": "event-type",
	"args": [ "..." ]
}
```

The `args` field may not be present or may be empty, but will never contain anything other than strings.  

Websockets are authenticated through JWTs which contain a list of permissions restricting control of the API. See [Websocket Authentication](websocket_auth.md) for more details.  

## Related wings source

- [Core websocket inbound handling](https://github.com/pterodactyl/wings/blob/1d5090957b63d6bee77dcf11f188115f19776325/router/websocket/websocket.go)

## Received by wings

### auth

Provides a JWT used to authenticate the connection. Do not send or accept any messages prior to authenticating the connection. More information on websocket JWT authentication is available [here](websocket_auth.md).

```json
{
	"event": "auth",
	"args": [
		"eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCIsImp0aSI6ImI1Yjk1ZGNmNTg2YjhmNzg1ZWZkYzQ3ZWM4ZDc0NmNlIn0.eyJpc3MiOiJodHRwOi8vbG9jYWxob3N0OjMwMDAiLCJhdWQiOlsiaHR0cDovL2xvY2FsaG9zdDo4MDgwIl0sImp0aSI6ImI1Yjk1ZGNmNTg2YjhmNzg1ZWZkYzQ3ZWM4ZDc0NmNlIiwiaWF0IjoxNzAwMDAwMDAwLCJuYmYiOjE3MDAwMDAwMDAsImV4cCI6MTcwMDAwMDYwMCwic2VydmVyX3V1aWQiOiIzZmQzZTEyOS0wZGMzLTRmMGUtOWEzNi02ODUyNmNhYmZiY2YiLCJwZXJtaXNzaW9ucyI6W10sInVzZXJfdXVpZCI6IjE5YTE3ZDM2LTM2YjMtNGFjNi05ZmJmLWQzYjcwOGFmMjA1NiIsInVzZXJfaWQiOjF9.JQnaXfZ0_T0mG8hvl0xiXLa6cw9OxcjbNLnWUwr63Ww"
	]
}
```

### send stats

Signals to wings the client would like to receive stats. This request persists for the entirety of the connection.

```json
{
	"event": "send stats",
    "args": []
}
```

### send logs

Signals to wings the client would like to receive logs. This request persists for the entirety of the connection.  

Restricted by the `control.console` permission.

```json
{
	"event": "send logs",
	"args": []
}
```

### set state

Tells wings to change the server's state. Available states are `start`, `stop`, `restart` or `kill`.  

Restricted by the `control.{start/stop/restart}` permissions.

```json
{
	"event": "set state",
	"args": [
		"stop"
	]
}
```

### send command

Tell wings to execute the specified command.

```json
{
	"event": "send command",
	"args": [
		"help"
	]
}
```

## Sent by wings

### auth success

Notifies the client that its authentication attempt succeeded.

```json
{
	"event": "auth success"
	"args": []
}
```

### status

Informs the client of the server's status. Typically sent right after authentication was successful.  

Available statuses are `offline`, `starting`, `running` and `stopping`.

```json
{
	"event": "status",
	"args": [
		"starting"
	]
}
```

### stats

Used to update the client about performance statistics and status of the server. Should only be sent if [`send stats`](#send-stats) has been received and if the server is not in an `offline` state.

The `args` array contains one string containing a serialized JSON object following this schema:

```json
{
	"memory_bytes": 1507557376, // RAM used
	"memory_limit_bytes": 8041996288, // total RAM available
	"cpu_absolute": 10.001, // CPU usage %
	"network": {
		"rx_bytes": 12220,
		"tx_bytes": 3136
	},
	"uptime": 400000,
	"state": "running", // status as defined in the `status` event
	"disk_bytes": 115489609
}
```

As such, the message will look like:

```json
{
	"event": "stats",
	"args": [
		"{ \"memory_bytes\": 1507557376, \"state\": \"running\", ... }"
	]
}
```

### console output

Sends normal output lines to the client, which may contain terminal color codes.

```json
{
	"event": "console output",
	"args": [
		"\u001b[33m\u001b[1m[Pterodactyl Daemon]:\u001b[39m Finished pulling Docker container image\u001b[0m"
	]
}
```

Do note wings will not buffer multiple output lines into the `args` array.

See also: [`install output`](#install-output)

### install output

Sends installation output to the client, which may contain terminal color codes.  

Restricted by the `admin.websocket.install` permission.

```json
{
	"event": "install output",
	"args": [
		"Status: Image is up to date for ghcr.io/pterodactyl/yolks:java_17"
	]
}
```

Do note wings will not buffer multiple output lines into the `args` array.

See also: [`console output`](#console-output)

### jwt error

Notifies the client they did not provide correct credentials while attempting to authenticate.  

Restricted by the `admin.websocket.errors` permission.  

```json
{
	"event": "jwt error",
	"args": [
		"Error message"
	]
}
```

### daemon error

Notifies the client an error the daemon has encountered an error. The error message will be a generic error message unless the client has the `admin.websocket.errors` permission.  

```json
{
	"event": "daemon error",
	"args": [
		"an unexpected error was encountered while handling this request"
	]
}
```

### token expiring

Sent to the client when the JWT expires in less than 60 seconds.  

Do note the expiration deadline is checked only every 30 seconds, so this event may take a while to be sent.  

Clients should always try to reauthenticate as soon as they receive this event in order to avoid issues mentionned in [`token expired`](#token-expired).


```json
{
	"event": "token expiring",
	"args": []
}
```

### token expired

Sent to the client when the JWT has expired.  

From my observations, this event is only sent by a task which checks JWTs expiration times every 30 seconds. If the client sends a message, but the JWT has expired, wings will send either a `jwt error` if the client has `admin.websockets.error` permission, or a generic `daemon error` otherwise. Clients should therefore try to reauthenticate whenever they get an error, then resend the message.  

```json
{
	"event": "token expired",
	"args": []
}
```
