// rubix-hello prints a known marker, stays alive for a bounded time, and exits 0.
//
// Usage: rubix-hello [seconds|fail]
//
//	seconds: how long to stay alive after printing (default 60)
//	fail:    print an error and exit 3
package main

import (
	"fmt"
	"os"
	"strconv"
	"time"
)

// main executes the rubix-hello command.
func main() {
	fmt.Println("rubix-ok")
	seconds := 60
	if len(os.Args) > 1 {
		if os.Args[1] == "fail" {
			fmt.Fprintln(os.Stderr, "rubix-fail")
			os.Exit(3)
		}
		if n, err := strconv.Atoi(os.Args[1]); err == nil {
			seconds = n
		}
	}
	time.Sleep(time.Duration(seconds) * time.Second)
}
