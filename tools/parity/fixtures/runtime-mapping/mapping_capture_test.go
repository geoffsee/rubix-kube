package config

import (
	"encoding/json"
	"fmt"
	"github.com/portainer/kubesolo/internal/runtime/cri"
	"testing"
)

type mappingInput struct {
	ID       string `json:"id"`
	Path     string `json:"path"`
	NodeName string `json:"node_name"`
	Hostname string `json:"hostname"`
	Endpoint string `json:"endpoint"`
	Changed  bool   `json:"changed"`
}

func TestRubixCapture(t *testing.T) {
	inputs := []mappingInput{
		{ID: "default", Path: "/var/lib/kubesolo", Hostname: "fixture-node"},
		{ID: "custom", Path: "/fixture/a/../state//", NodeName: "  Talos-CP-1  ", Hostname: "unused", Changed: true},
		{ID: "empty-path", Path: "", Hostname: "fixture-node"},
		{ID: "relative-path", Path: "relative/../state", Hostname: "fixture-node"},
		{ID: "raw-hostname", Path: "/fixture", NodeName: " ", Hostname: " MIXED-Host "},
		{ID: "empty-hostname", Path: "/fixture", Hostname: ""},
		{ID: "external-path", Path: "/fixture", Hostname: "fixture-node", Endpoint: "  /run/crio/crio.sock  ", Changed: true},
		{ID: "external-url", Path: "/fixture", Hostname: "fixture-node", Endpoint: "unix:///run/a/../runtime.sock"},
		{ID: "whitespace-endpoint", Path: "/fixture", Hostname: "fixture-node", Endpoint: " \t "},
		{ID: "relative-endpoint", Path: "/fixture", Hostname: "fixture-node", Endpoint: "run/runtime.sock"},
		{ID: "non-unix-endpoint", Path: "/fixture", Hostname: "fixture-node", Endpoint: "tcp://127.0.0.1:1234"},
		{ID: "unix-host-endpoint", Path: "/fixture", Hostname: "fixture-node", Endpoint: "unix://localhost/run/runtime.sock"},
		{ID: "root-endpoint", Path: "/fixture", Hostname: "fixture-node", Endpoint: "unix:///"},
	}
	out := make([]map[string]any, 0, len(inputs))
	for _, input := range inputs {
		cfg := Defaults()
		cfg.Path = input.Path
		cfg.Kubernetes.NodeName = input.NodeName
		endpoint, err := cri.Resolve(input.Endpoint)
		record := map[string]any{"input": input, "resolved": endpoint, "error": "", "embedded": nil}
		if err != nil {
			record["error"] = err.Error()
			out = append(out, record)
			continue
		}
		probe := Probe{Hostname: input.Hostname, NodeIP: "192.0.2.10", LoadBalancerIP: "192.0.2.11", MTU: 1450, RuntimeEndpoint: endpoint}
		if input.Changed {
			// These intentionally conflicting desired values prove BuildEmbedded consumes supplied probes.
			cfg.Network.NodeIP = "198.51.100.1"
			cfg.Network.MTU = 9000
			cfg.Network.LoadBalancer.IP = "198.51.100.2"
			disabled := false
			cfg.Runtime.ContainerMode = &disabled
			cfg.Runtime.Endpoint = "unix:///ignored/config.sock"
			probe.NodeIP = "2001:db8::10"
			probe.NodeIPPinned = true
			probe.LoadBalancerIP = "2001:db8::11"
			probe.MTU = 1280
			probe.MTUPinned = true
			probe.ContainerMode = true
			cfg.Network.LoadBalancer.Enabled = false
			cfg.Storage.LocalPath.Enabled = false
			cfg.Network.DisableIPv6 = true
			cfg.Kubernetes.APIServer.ExtraSANs = []string{"fixture.example", "192.0.2.55"}
			cfg.Kubernetes.Kubelet.CPUManager.Policy = "static"
			cfg.Kubernetes.Kubelet.CPUManager.ReservedCPUs = "0-1"
			cfg.Kubernetes.Kubelet.CPUManager.PolicyOptions = map[string]string{"full-pcpus-only": "true"}
			cfg.Kubernetes.Kubelet.SystemReserved = map[string]string{"cpu": "200m", "memory": "128Mi"}
			cfg.D2K.Enabled = true
			cfg.D2K.Namespace = "fixture-d2k"
			cfg.Metrics.Enabled = true
			cfg.Metrics.BindAddress = "127.0.0.1:19105"
			cfg.Portainer.EdgeID = "synthetic-id"
			cfg.Portainer.EdgeKey = "synthetic-key"
			cfg.Portainer.Image = "fixture.invalid/agent:custom"
		}
		record["embedded"] = BuildEmbedded(cfg, probe)
		out = append(out, record)
	}
	raw, err := json.Marshal(out)
	if err != nil {
		t.Fatal(err)
	}
	fmt.Printf("RUBIX_CAPTURE %s\n", raw)
}
