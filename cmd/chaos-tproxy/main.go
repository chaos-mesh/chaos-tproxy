package main

import (
	"context"
	"os"

	"github.com/chaos-mesh/chaos-tproxy/pkg/cli"
)

func main() {
	os.Exit(cli.Run(context.Background(), os.Args[1:], os.Stdin, os.Stdout, os.Stderr))
}
