ARG TPROXY_IMAGE=chaos-mesh/tproxy:recovery
FROM ${TPROXY_IMAGE} AS tproxy

FROM golang:1.25.11-bookworm AS build
COPY fixture.go /src/fixture.go
RUN CGO_ENABLED=0 go build -o /fixture /src/fixture.go

FROM ghcr.io/chaos-mesh/chaos-daemon:v2.8.4
COPY --from=build /fixture /fixture
COPY --from=tproxy /tproxy /usr/local/bin/tproxy
ENTRYPOINT ["/fixture"]
