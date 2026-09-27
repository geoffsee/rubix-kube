package containerd

import (
	"bytes"
	"encoding/json"
	"fmt"
	"github.com/pelletier/go-toml"
	"os"
	"path/filepath"
	"testing"
)

func TestRubixCapture(t *testing.T) {
	must := func(e error) {
		t.Helper()
		if e != nil {
			t.Fatal(e)
		}
	}
	root := "/tmp/rubix-runtime"
	must(os.MkdirAll(root, 0700))
	if _, e := os.Stat("/run/systemd/private"); !os.IsNotExist(e) {
		t.Fatal("unexpected systemd fixture state")
	}
	s := &service{containerdRootDir: filepath.Join(root, "root"), containerdStateDir: "/fixture/state", containerdSocketFile: "/fixture/containerd.sock", containerdCNIPluginsDir: "/fixture/cni/bin", containerdRegistryConfigDir: "/fixture/registry", crunBinaryFile: "/fixture/crun", containerdShimBinaryFile: "/fixture/containerd-shim-runc-v2", containerdConfigFile: filepath.Join(root, "config.toml")}
	config := s.generateContainerdConfig()
	must(s.writeContainerdConfigFile())
	raw, e := os.ReadFile(s.containerdConfigFile)
	must(e)
	tree, e := toml.LoadBytes(raw)
	must(e)
	must(s.writeContainerdConfigFile())
	again, e := os.ReadFile(s.containerdConfigFile)
	must(e)
	checks := map[string]any{"repeat_equal": bytes.Equal(raw, again), "tmpfs_snapshotter": pickSnapshotter(root), "missing_parent_snapshotter": pickSnapshotter("/tmp/rubix-runtime/absent/deeper"), "systemd_cgroup": useSystemdCgroup()}
	s.containerdConfigFile = root
	checks["output_directory_fails"] = s.writeContainerdConfigFile() != nil
	s.containerdConfigFile = filepath.Join(root, "absent", "config.toml")
	checks["missing_parent_fails"] = s.writeContainerdConfigFile() != nil
	encoded, e := json.Marshal(map[string]any{"component": "containerd", "config": config, "rendered_config": tree.ToMap(), "toml": string(raw), "checks": checks})
	must(e)
	fmt.Printf("RUBIX_CAPTURE %s\n", encoded)
}
