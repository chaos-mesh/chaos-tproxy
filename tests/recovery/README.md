# Test controller recovery

Build tproxy, then run the lifecycle test with Docker and Python 3.

```sh
docker build -t chaos-mesh/tproxy:recovery .
bash tests/recovery/run.sh chaos-mesh/tproxy:recovery
```

The test uses a disposable container with its own network namespace. It grants network and mount capabilities and permits the network sysctl writes required by tproxy. It does not use host networking or a Kubernetes cluster.

The original interface retains an IPv6 address while tproxy moves its IPv4 address. The test checks response replacement, pause, resume, active rule reload, stopping with a role, Server role reload, and restoration of the original IPv4 address and routes.

The script removes its container and fixture image on exit. It saves the container log beside the script.
