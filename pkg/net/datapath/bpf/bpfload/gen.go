// Package bpfload contains bpf2go-generated bindings for the datapath
// program in ../program/redirect.c.
package bpfload

//go:generate bpf2go -cc clang -target bpfel -cflags "-O2 -g -Wall -Werror -I/usr/include/x86_64-linux-gnu -I/usr/include" Datapath ../program/redirect.c
