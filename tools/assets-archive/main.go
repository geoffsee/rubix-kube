// Synthetic serialization fixture: no registry payloads and no image execution.
package main

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"runtime/debug"

	"github.com/google/go-containerregistry/pkg/crane"
	v1 "github.com/google/go-containerregistry/pkg/v1"
	"github.com/google/go-containerregistry/pkg/v1/empty"
	"github.com/google/go-containerregistry/pkg/v1/mutate"
	"github.com/google/go-containerregistry/pkg/v1/tarball"
)

func layer(label string) (v1.Layer, error) {
	var raw bytes.Buffer
	tw := tar.NewWriter(&raw)
	body := bytes.Repeat([]byte(label+"\n"), 1025)
	if err := tw.WriteHeader(&tar.Header{Name: "fixture.txt", Mode: 0644, Size: int64(len(body)), Typeflag: tar.TypeReg}); err != nil {
		return nil, err
	}
	if _, err := tw.Write(body); err != nil {
		return nil, err
	}
	if err := tw.Close(); err != nil {
		return nil, err
	}
	return tarball.LayerFromOpener(func() (io.ReadCloser, error) { return io.NopCloser(bytes.NewReader(raw.Bytes())), nil })
}
func run() error {
	if len(os.Args) != 2 {
		return fmt.Errorf("one output directory required")
	}
	if runtime.Version() != "go1.26.2" {
		return fmt.Errorf("wrong Go toolchain: %s", runtime.Version())
	}
	info, ok := debug.ReadBuildInfo()
	if !ok {
		return fmt.Errorf("missing module provenance")
	}
	found := false
	for _, dep := range info.Deps {
		if dep.Path == "github.com/google/go-containerregistry" {
			found = dep.Version == "v0.21.5" && dep.Sum == "h1:KTJG9Pn/jC0VdZR6ctV3/jcN+q6/Iqlx0sTVz3ywZlM=" && dep.Replace == nil
		}
	}
	if !found {
		return fmt.Errorf("wrong crane module")
	}
	a, err := layer("synthetic-a")
	if err != nil {
		return err
	}
	b, err := layer("synthetic-b")
	if err != nil {
		return err
	}
	for _, row := range []struct{ name, arch string }{{"amd64-repeated", "amd64"}, {"armv7-unresolved", "arm"}} {
		img, err := mutate.AppendLayers(empty.Image, a, b, a)
		if err != nil {
			return err
		}
		cfg, err := img.ConfigFile()
		if err != nil {
			return err
		}
		cfg.OS = "linux"
		cfg.Architecture = row.arch
		img, err = mutate.ConfigFile(img, cfg)
		if err != nil {
			return err
		}
		path := filepath.Join(os.Args[1], row.name+".tar")
		// Same Save/MultiSave -> tarball writer path as the pinned crane pull default.
		if err := crane.Save(img, "example.invalid/fixture:"+row.name, path); err != nil {
			return err
		}
		file, err := os.Open(path)
		if err != nil {
			return err
		}
		raw, err := io.ReadAll(io.LimitReader(file, 1024*1024+1))
		closeErr := file.Close()
		if err != nil {
			return err
		}
		if closeErr != nil {
			return closeErr
		}
		if len(raw) > 1024*1024 {
			return fmt.Errorf("fixture size bound")
		}
		var encoded bytes.Buffer
		gz := gzip.NewWriter(&encoded)
		if _, err := gz.Write(raw); err != nil {
			return err
		}
		if err := gz.Close(); err != nil {
			return err
		}
		if err := os.WriteFile(path+".gz", encoded.Bytes(), 0600); err != nil {
			return err
		}
	}
	fmt.Println("RUBIX_PRODUCER go1.26.2 github.com/google/go-containerregistry v0.21.5 h1:KTJG9Pn/jC0VdZR6ctV3/jcN+q6/Iqlx0sTVz3ywZlM=")
	return nil
}
func main() {
	if err := run(); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
