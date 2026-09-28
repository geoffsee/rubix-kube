// Synthetic layer serialization oracle; never imports or executes image contents.
package main

import (
	"archive/tar"
	"bytes"
	"compress/gzip"
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"runtime/debug"

	"github.com/google/go-containerregistry/pkg/compression"
	"github.com/google/go-containerregistry/pkg/crane"
	v1 "github.com/google/go-containerregistry/pkg/v1"
	"github.com/google/go-containerregistry/pkg/v1/empty"
	"github.com/google/go-containerregistry/pkg/v1/mutate"
	"github.com/google/go-containerregistry/pkg/v1/tarball"
	"github.com/google/go-containerregistry/pkg/v1/types"
	"github.com/klauspost/compress/zstd"
)

func digest(b []byte) string { h := sha256.Sum256(b); return hex.EncodeToString(h[:]) }
func rawLayer(label string) ([]byte, error) {
	var body []byte
	if label == "a" {
		body = bytes.Repeat([]byte("synthetic-layer-a\n"), 2049)
	} else {
		for i := 0; i < 1100; i++ {
			h := sha256.Sum256([]byte(fmt.Sprintf("synthetic-layer-b:%d", i)))
			body = append(body, h[:]...)
		}
	}
	var raw bytes.Buffer
	tw := tar.NewWriter(&raw)
	if err := tw.WriteHeader(&tar.Header{Name: "fixture.txt", Mode: 0644, Size: int64(len(body)), Typeflag: tar.TypeReg}); err != nil {
		return nil, err
	}
	if _, err := tw.Write(body); err != nil {
		return nil, err
	}
	if err := tw.Close(); err != nil {
		return nil, err
	}
	return raw.Bytes(), nil
}
func encode(raw []byte, codec string) ([]byte, error) {
	var b bytes.Buffer
	if codec == "gzip" {
		w := gzip.NewWriter(&b)
		if _, e := w.Write(raw); e != nil {
			return nil, e
		}
		if e := w.Close(); e != nil {
			return nil, e
		}
	} else {
		w, e := zstd.NewWriter(&b, zstd.WithEncoderConcurrency(1), zstd.WithEncoderCRC(true))
		if e != nil {
			return nil, e
		}
		if _, e = w.Write(raw); e != nil {
			return nil, e
		}
		if e = w.Close(); e != nil {
			return nil, e
		}
	}
	return b.Bytes(), nil
}

type observation struct {
	Stored  string `json:"stored_sha256"`
	Size    int    `json:"stored_bytes"`
	DiffID  string `json:"diff_id"`
	Decoded []byte `json:"decoded_base64"`
	Frames  int    `json:"frames"`
	Codec   string `json:"codec"`
}

func layer(label, codec string, concat bool) (v1.Layer, observation, error) {
	raw, e := rawLayer(label)
	if e != nil {
		return nil, observation{}, e
	}
	parts := [][]byte{raw}
	if concat {
		parts = [][]byte{raw[:len(raw)/2], raw[len(raw)/2:]}
	}
	var stored []byte
	for _, part := range parts {
		b, e := encode(part, codec)
		if e != nil {
			return nil, observation{}, e
		}
		stored = append(stored, b...)
	}
	comp, media := compression.GZip, types.OCILayer
	if codec == "zstd" {
		comp, media = compression.ZStd, types.OCILayerZStd
	}
	l, e := tarball.LayerFromOpener(func() (io.ReadCloser, error) { return io.NopCloser(bytes.NewReader(stored)), nil }, tarball.WithCompression(comp), tarball.WithMediaType(media))
	if e != nil {
		return nil, observation{}, e
	}
	// Independently decode through upstream's layer abstraction and compare every raw byte.
	r, e := l.Uncompressed()
	if e != nil {
		return nil, observation{}, e
	}
	decoded, e := io.ReadAll(io.LimitReader(r, 1024*1024+1))
	ce := r.Close()
	if e != nil {
		return nil, observation{}, e
	}
	if ce != nil {
		return nil, observation{}, ce
	}
	if !bytes.Equal(decoded, raw) {
		return nil, observation{}, fmt.Errorf("upstream decoded stream differs")
	}
	diff, e := l.DiffID()
	if e != nil {
		return nil, observation{}, e
	}
	hash, e := l.Digest()
	if e != nil {
		return nil, observation{}, e
	}
	size, e := l.Size()
	if e != nil {
		return nil, observation{}, e
	}
	if diff.String() != "sha256:"+digest(raw) || hash.String() != "sha256:"+digest(stored) || size != int64(len(stored)) {
		return nil, observation{}, fmt.Errorf("upstream identity mismatch")
	}
	return l, observation{digest(stored), len(stored), digest(raw), decoded, len(parts), codec}, nil
}
func run() error {
	if len(os.Args) != 2 {
		return fmt.Errorf("one output directory required")
	}
	if runtime.Version() != "go1.26.2" {
		return fmt.Errorf("wrong Go toolchain")
	}
	info, ok := debug.ReadBuildInfo()
	if !ok {
		return fmt.Errorf("module provenance missing")
	}
	found := false
	for _, d := range info.Deps {
		if d.Path == "github.com/google/go-containerregistry" {
			found = d.Version == "v0.21.5" && d.Sum == "h1:KTJG9Pn/jC0VdZR6ctV3/jcN+q6/Iqlx0sTVz3ywZlM=" && d.Replace == nil
		}
	}
	if !found {
		return fmt.Errorf("wrong crane module")
	}
	rows := map[string][]observation{}
	for _, codec := range []string{"gzip", "zstd"} {
		for _, concat := range []bool{false, true} {
			name := codec
			if concat {
				name += "-concat"
			}
			a, ao, e := layer("a", codec, concat)
			if e != nil {
				return e
			}
			b, bo, e := layer("b", codec, concat)
			if e != nil {
				return e
			}
			img, e := mutate.AppendLayers(empty.Image, a, b, a)
			if e != nil {
				return e
			}
			cfg, e := img.ConfigFile()
			if e != nil {
				return e
			}
			cfg.OS = "linux"
			cfg.Architecture = "amd64"
			img, e = mutate.ConfigFile(img, cfg)
			if e != nil {
				return e
			}
			path := filepath.Join(os.Args[1], name+".tar")
			if e = crane.Save(img, "example.invalid/layer:"+name, path); e != nil {
				return e
			}
			f, e := os.Open(path)
			if e != nil {
				return e
			}
			raw, e := io.ReadAll(io.LimitReader(f, 1024*1024+1))
			ce := f.Close()
			if e != nil {
				return e
			}
			if ce != nil {
				return ce
			}
			if len(raw) > 1024*1024 {
				return fmt.Errorf("archive limit")
			}
			encoded, e := encode(raw, "gzip")
			if e != nil {
				return e
			}
			if e = os.WriteFile(path+".gz", encoded, 0600); e != nil {
				return e
			}
			rows[name] = []observation{ao, bo, ao}
		}
	}
	raw, e := json.Marshal(rows)
	if e != nil {
		return e
	}
	if e = os.WriteFile(filepath.Join(os.Args[1], "upstream.json"), raw, 0600); e != nil {
		return e
	}
	fmt.Println("RUBIX_PRODUCER go1.26.2 github.com/google/go-containerregistry v0.21.5 h1:KTJG9Pn/jC0VdZR6ctV3/jcN+q6/Iqlx0sTVz3ywZlM=")
	return nil
}
func main() {
	if e := run(); e != nil {
		fmt.Fprintln(os.Stderr, e)
		os.Exit(1)
	}
}
