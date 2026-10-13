package main

import (
	"bufio"
	"bytes"
	"encoding/json"
	"io"
	"log"
	"net/http"
	"os"
	"os/exec"
	"sync"
)

func main() {
	cmd := exec.Command("/usr/local/bin/tproxy", "-i", "-vv")
	in, err := cmd.StdinPipe()
	if err != nil {
		log.Fatal(err)
	}
	out, err := cmd.StdoutPipe()
	if err != nil {
		log.Fatal(err)
	}
	cmd.Stderr = os.Stderr
	if err := cmd.Start(); err != nil {
		log.Fatal(err)
	}
	reader := bufio.NewReader(out)
	var mutex sync.Mutex
	http.HandleFunc("/config", func(w http.ResponseWriter, r *http.Request) {
		data, err := io.ReadAll(r.Body)
		if err != nil {
			http.Error(w, err.Error(), 500)
			return
		}
		mutex.Lock()
		defer mutex.Unlock()
		req, err := http.NewRequest(http.MethodPut, "http://tproxy/", bytes.NewReader(data))
		if err != nil {
			http.Error(w, err.Error(), 500)
			return
		}
		if err := req.Write(in); err != nil {
			http.Error(w, err.Error(), 500)
			return
		}
		resp, err := http.ReadResponse(reader, req)
		if err != nil {
			http.Error(w, err.Error(), 500)
			return
		}
		defer resp.Body.Close()
		w.WriteHeader(resp.StatusCode)
		io.Copy(w, resp.Body)
	})
	http.HandleFunc("/state", func(w http.ResponseWriter, r *http.Request) {
		state := map[string]json.RawMessage{}
		for _, command := range []string{"address", "route"} {
			data, err := exec.Command("ip", "-j", command, "show").Output()
			if err != nil {
				http.Error(w, err.Error(), 500)
				return
			}
			state[command] = data
		}
		json.NewEncoder(w).Encode(state)
	})
	origin := http.NewServeMux()
	origin.HandleFunc("/", func(w http.ResponseWriter, r *http.Request) { io.WriteString(w, "original") })
	go func() { log.Fatal(http.ListenAndServe(":8080", origin)) }()
	log.Fatal(http.ListenAndServe(":18080", nil))
}
