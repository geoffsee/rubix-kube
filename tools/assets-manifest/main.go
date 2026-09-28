// Offline crane serialization oracle. Image payloads are never extracted or executed.
package main

import (
	"compress/gzip"
	"crypto/sha256"
	_ "embed"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"runtime/debug"
	"syscall"

	"github.com/google/go-containerregistry/pkg/crane"
	v1 "github.com/google/go-containerregistry/pkg/v1"
	"github.com/google/go-containerregistry/pkg/v1/partial"
	"github.com/google/go-containerregistry/pkg/v1/types"
)

//go:embed inputs.json
var pinned []byte

type blob struct {
	root       string
	descriptor v1.Descriptor
}

func (b blob) Digest() (v1.Hash, error)            { return b.descriptor.Digest, nil }
func (b blob) Size() (int64, error)                { return b.descriptor.Size, nil }
func (b blob) MediaType() (types.MediaType, error) { return b.descriptor.MediaType, nil }
func (b blob) Compressed() (io.ReadCloser, error) {
	return regular(filepath.Join(b.root, "blobs", b.descriptor.Digest.Hex))
}

type image struct {
	root             string
	manifest, config []byte
	parsed           v1.Manifest
}

func (i image) RawManifest() ([]byte, error)        { return i.manifest, nil }
func (i image) RawConfigFile() ([]byte, error)      { return i.config, nil }
func (i image) MediaType() (types.MediaType, error) { return i.parsed.MediaType, nil }
func (i image) LayerByDigest(h v1.Hash) (partial.CompressedLayer, error) {
	for _, d := range i.parsed.Layers {
		if d.Digest == h {
			return blob{i.root, d}, nil
		}
	}
	return nil, fmt.Errorf("unlisted blob %s", h)
}
func regular(path string) (*os.File, error) {
	fd, e := syscall.Open(path, syscall.O_RDONLY|syscall.O_NOFOLLOW|syscall.O_NONBLOCK, 0)
	if e != nil {
		return nil, e
	}
	f := os.NewFile(uintptr(fd), path)
	s, e := f.Stat()
	if e != nil {
		f.Close()
		return nil, e
	}
	if !s.Mode().IsRegular() {
		f.Close()
		return nil, fmt.Errorf("regular file required: %s", path)
	}
	return f, nil
}
func checked(path, hash string, size int64) ([]byte, error) {
	f, e := regular(path)
	if e != nil {
		return nil, e
	}
	defer f.Close()
	s, e := f.Stat()
	if e != nil {
		return nil, e
	}
	if !s.Mode().IsRegular() || s.Size() != size {
		return nil, fmt.Errorf("input size %s", path)
	}
	b, e := io.ReadAll(io.LimitReader(f, size+1))
	if e != nil {
		return nil, e
	}
	h := sha256.Sum256(b)
	if int64(len(b)) != size || hex.EncodeToString(h[:]) != hash {
		return nil, fmt.Errorf("input identity %s", path)
	}
	return b, nil
}
func run() error {
	if len(os.Args) != 3 {
		return fmt.Errorf("usage: producer INPUT_CACHE OUTPUT_DIRECTORY")
	}
	if runtime.Version() != "go1.26.2" {
		return fmt.Errorf("pinned Go version")
	}
	info, ok := debug.ReadBuildInfo()
	if !ok {
		return fmt.Errorf("build info")
	}
	found := false
	for _, d := range info.Deps {
		if d.Path == "github.com/google/go-containerregistry" {
			if d.Version != "v0.21.5" || d.Sum != "h1:KTJG9Pn/jC0VdZR6ctV3/jcN+q6/Iqlx0sTVz3ywZlM=" || d.Replace != nil {
				return fmt.Errorf("crane pin")
			}
			found = true
		}
	}
	if !found {
		return fmt.Errorf("missing crane")
	}
	var p struct {
		ManifestSHA   string `json:"manifest_sha256"`
		ManifestBytes int64  `json:"manifest_bytes"`
		ConfigSHA     string `json:"config_sha256"`
		ConfigBytes   int64  `json:"config_bytes"`
		RepoTag       string `json:"repo_tag"`
	}
	if e := json.Unmarshal(pinned, &p); e != nil {
		return e
	}
	root, out := os.Args[1], os.Args[2]
	m, e := checked(filepath.Join(root, "arm64-manifest.json"), p.ManifestSHA, p.ManifestBytes)
	if e != nil {
		return e
	}
	c, e := checked(filepath.Join(root, "arm64-config.json"), p.ConfigSHA, p.ConfigBytes)
	if e != nil {
		return e
	}
	var parsed v1.Manifest
	if e = json.Unmarshal(m, &parsed); e != nil {
		return e
	}
	for _, d := range parsed.Layers {
		if _, e = checked(filepath.Join(root, "blobs", d.Digest.Hex), d.Digest.Hex, d.Size); e != nil {
			return e
		}
	}
	img, e := partial.CompressedToImage(image{root, m, c, parsed})
	if e != nil {
		return e
	}
	// Own a new private directory before crane opens its tar output.
	if e = os.Mkdir(out, 0700); e != nil {
		return e
	}
	tarPath := filepath.Join(out, "coredns.tar")
	if e = crane.Save(img, p.RepoTag, tarPath); e != nil {
		return e
	}
	f, e := os.Open(tarPath)
	if e != nil {
		return e
	}
	defer f.Close()
	target, e := os.OpenFile(filepath.Join(out, "coredns.tar.gz"), os.O_CREATE|os.O_EXCL|os.O_WRONLY, 0600)
	if e != nil {
		return e
	}
	gz := gzip.NewWriter(target)
	_, copyError := io.Copy(gz, f)
	gzipError := gz.Close()
	closeError := target.Close()
	if copyError != nil {
		return copyError
	}
	if gzipError != nil {
		return gzipError
	}
	if closeError != nil {
		return closeError
	}
	if e = os.Remove(tarPath); e != nil {
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
