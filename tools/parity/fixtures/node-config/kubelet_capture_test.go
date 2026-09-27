package kubelet

import (
	"bytes"
	"encoding/json"
	"fmt"
	"github.com/portainer/kubesolo/types"
	"github.com/spf13/cobra"
	"os"
	"path/filepath"
	"sigs.k8s.io/yaml"
	"strings"
	"testing"
)

func TestRubixCapture(t *testing.T) {
	must := func(e error) {
		t.Helper()
		if e != nil {
			t.Fatal(e)
		}
	}
	root := "/tmp/rubix-node"
	must(os.MkdirAll(root, 0700))
	resolver, e := os.ReadFile("/etc/resolv.conf")
	must(e)
	nameservers := []string{}
	for _, line := range strings.Split(string(resolver), "\n") {
		fields := strings.Fields(line)
		if len(fields) > 0 && fields[0] == "nameserver" {
			nameservers = append(nameservers, fields[1])
		}
	}
	if len(nameservers) != 1 || nameservers[0] != "192.0.2.53" {
		t.Fatal("synthetic resolver control not active")
	}
	if _, e := os.Stat("/run/systemd/private"); !os.IsNotExist(e) {
		t.Fatal("unexpected systemd fixture state")
	}
	s := &service{kubeletDir: root, kubeletConfigDir: root, kubeletConfigFile: filepath.Join(root, "kubelet.yaml"), kubeletKubeConfigFile: "/fixture/node.kubeconfig", runtimeEndpoint: "unix:///fixture/runtime.sock", caFile: "/fixture/ca.crt", certFile: "/fixture/node.crt", keyFile: "/fixture/node.key", nodeName: "fixture-node", nodeIP: "192.0.2.8"}
	variants := map[string]any{}
	for _, name := range []string{"host_default", "container_default", "container_static", "reported_systemd", "reported_cgroupfs"} {
		s.containerMode = name != "host_default"
		s.runtimeCgroupDriver = ""
		s.cpuManager = types.CPUManagerConfig{}
		s.systemReserved = nil
		if name == "container_static" {
			s.cpuManager = types.CPUManagerConfig{Policy: "static", ReservedCPUs: "0-1", PolicyOptions: map[string]string{"full-pcpus-only": "true"}}
			s.systemReserved = map[string]string{"cpu": "200m", "memory": "128Mi"}
		}
		if name == "reported_systemd" {
			s.runtimeCgroupDriver = "systemd"
		}
		if name == "reported_cgroupfs" {
			s.runtimeCgroupDriver = "cgroupfs"
		}
		config := s.generateKubeletConfig()
		must(s.writeKubeletConfigFile())
		raw, e := os.ReadFile(s.kubeletConfigFile)
		must(e)
		parsed, e := yaml.YAMLToJSON(raw)
		must(e)
		var rendered any
		must(json.Unmarshal(parsed, &rendered))
		must(s.writeKubeletConfigFile())
		again, e := os.ReadFile(s.kubeletConfigFile)
		must(e)
		variants[name] = map[string]any{"config": config, "rendered_config": rendered, "yaml": string(raw), "repeat_equal": bytes.Equal(raw, again)}
	}
	s.containerMode = true
	s.runtimeCgroupDriver = "cgroupfs"
	checkpoint := filepath.Join(root, cpuManagerCheckpointFile)
	checkpointState := func() string {
		value, err := os.ReadFile(checkpoint)
		if os.IsNotExist(err) {
			return "absent"
		}
		must(err)
		if bytes.Equal(value, []byte("synthetic checkpoint")) {
			return "unchanged"
		}
		return "changed"
	}
	transitions := map[string]any{}
	for _, name := range []string{"same", "policy", "options", "reserved", "unrelated", "malformed_previous"} {
		s.cpuManager = types.CPUManagerConfig{Policy: "static", ReservedCPUs: "0", PolicyOptions: map[string]string{"full-pcpus-only": "true"}}
		s.systemReserved = nil
		must(s.writeKubeletConfigFile())
		must(os.WriteFile(checkpoint, []byte("synthetic checkpoint"), 0600))
		switch name {
		case "policy":
			s.cpuManager.Policy = "none"
		case "options":
			s.cpuManager.PolicyOptions["full-pcpus-only"] = "false"
		case "reserved":
			s.cpuManager.ReservedCPUs = "1"
		case "unrelated":
			s.systemReserved = map[string]string{"memory": "128Mi"}
		case "malformed_previous":
			must(os.WriteFile(s.kubeletConfigFile, []byte("[invalid"), 0600))
		}
		must(s.writeKubeletConfigFile())
		transitions[name] = checkpointState()
	}
	must(os.WriteFile(checkpoint, []byte("corrupt but present"), 0600))
	checkpointNegative := checkpointState()
	args := map[string]any{}
	for _, ip := range []string{"192.0.2.8", "2001:db8::8", "127.0.0.1", "bad"} {
		s.nodeIP = ip
		s.disableIPv6 = true
		var captured []string
		cmd := &cobra.Command{Use: "fixture", DisableFlagParsing: true, RunE: func(_ *cobra.Command, a []string) error { captured = append([]string{}, a...); return nil }}
		s.configureKubeletArgs(cmd)
		must(cmd.Execute())
		args[ip] = captured
	}
	s.kubeletConfigFile = root
	outputFailure := s.writeKubeletConfigFile() != nil
	s.kubeletConfigDir = filepath.Join(root, "blocked")
	must(os.WriteFile(s.kubeletConfigDir, []byte("not directory"), 0600))
	s.kubeletConfigFile = filepath.Join(s.kubeletConfigDir, "config")
	directoryFailure := s.writeKubeletConfigFile() != nil
	encoded, e := json.Marshal(map[string]any{"component": "kubelet", "synthetic_resolver": nameservers, "variants": variants, "checkpoint_states": transitions, "checkpoint_negative_control": checkpointNegative, "args": args, "failures": map[string]bool{"output_directory": outputFailure, "parent_file": directoryFailure}})
	must(e)
	fmt.Printf("RUBIX_CAPTURE %s\n", encoded)
}
