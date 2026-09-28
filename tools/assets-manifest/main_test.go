package main

import (
	"os"
	"path/filepath"
	"syscall"
	"testing"
)

func TestRegularRejectsSpecialAndSymlinkInputs(t *testing.T) {
	dir := t.TempDir()
	regularPath := filepath.Join(dir, "regular")
	if e := os.WriteFile(regularPath, []byte("retained"), 0600); e != nil {
		t.Fatal(e)
	}
	link := filepath.Join(dir, "link")
	if e := os.Symlink(regularPath, link); e != nil {
		t.Fatal(e)
	}
	fifo := filepath.Join(dir, "fifo")
	if e := syscall.Mkfifo(fifo, 0600); e != nil {
		t.Fatal(e)
	}
	for _, path := range []string{link, fifo, dir} {
		if f, e := regular(path); e == nil {
			f.Close()
			t.Fatalf("accepted %s", path)
		}
	}
	f, e := regular(regularPath)
	if e != nil {
		t.Fatal(e)
	}
	f.Close()
}

func TestProducerNeverTruncatesExistingOutput(t *testing.T) {
	cache := os.Getenv("RUBIX_MANIFEST_INPUTS")
	if cache == "" {
		t.Skip("explicit retained fixture cache required")
	}
	dir := t.TempDir()
	sentinel := filepath.Join(dir, "sentinel")
	if e := os.WriteFile(sentinel, []byte("preserve"), 0600); e != nil {
		t.Fatal(e)
	}
	if e := os.Symlink(sentinel, filepath.Join(dir, "coredns.tar")); e != nil {
		t.Fatal(e)
	}
	original := os.Args
	defer func() { os.Args = original }()
	os.Args = []string{"producer", cache, dir}
	if e := run(); e == nil {
		t.Fatal("accepted existing output directory")
	}
	raw, e := os.ReadFile(sentinel)
	if e != nil {
		t.Fatal(e)
	}
	if string(raw) != "preserve" {
		t.Fatal("output target changed")
	}
}
