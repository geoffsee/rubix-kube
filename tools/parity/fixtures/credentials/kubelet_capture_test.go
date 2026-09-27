package kubelet

import (
	"bytes"
	"encoding/json"
	"fmt"
	"k8s.io/client-go/tools/clientcmd"
	"os"
	"path/filepath"
	"testing"
)

func TestRubixCapture(t *testing.T) {
	dir := t.TempDir()
	s := &service{nodeIP: "192.0.2.8", nodeName: "fixture-node", caFile: "/fixture/ca.crt", certFile: "/fixture/node.crt", keyFile: "/fixture/node.key", kubeletKubeConfigFile: filepath.Join(dir, "node.config")}
	must := func(err error) {
		t.Helper()
		if err != nil {
			t.Fatal(err)
		}
	}
	must(s.generateKubeletKubeconfig())
	config, err := clientcmd.LoadFromFile(s.kubeletKubeConfigFile)
	must(err)
	info, err := os.Stat(s.kubeletKubeConfigFile)
	must(err)
	first, err := os.ReadFile(s.kubeletKubeConfigFile)
	must(err)
	must(s.generateKubeletKubeconfig())
	second, err := os.ReadFile(s.kubeletKubeConfigFile)
	must(err)
	_, caErr := os.Stat(s.caFile)
	_, certErr := os.Stat(s.certFile)
	_, keyErr := os.Stat(s.keyFile)
	checks := map[string]any{"kubeconfig_mode": fmt.Sprintf("%04o", info.Mode().Perm()), "repeat_equal": bytes.Equal(first, second), "missing_referenced_credentials_accepted": os.IsNotExist(caErr) && os.IsNotExist(certErr) && os.IsNotExist(keyErr)}
	s.nodeName = "changed-node"
	must(s.generateKubeletKubeconfig())
	changed, err := clientcmd.LoadFromFile(s.kubeletKubeConfigFile)
	must(err)
	checks["refreshes_identity"] = changed.CurrentContext == "system:node:changed-node@kubernetes"
	s.kubeletKubeConfigFile = dir
	checks["output_directory_fails"] = s.generateKubeletKubeconfig() != nil
	s.kubeletKubeConfigFile = filepath.Join(dir, "absent", "node.config")
	checks["missing_parent_fails"] = s.generateKubeletKubeconfig() != nil
	encoded, err := json.Marshal(map[string]any{"component": "kubelet", "path_kubeconfig": config, "checks": checks})
	must(err)
	fmt.Printf("RUBIX_CAPTURE %s\n", encoded)
}
