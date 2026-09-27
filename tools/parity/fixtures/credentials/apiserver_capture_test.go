package apiserver

import (
	"bytes"
	"crypto/x509"
	"encoding/json"
	"encoding/pem"
	"fmt"
	"k8s.io/client-go/tools/clientcmd"
	"os"
	"path/filepath"
	"testing"
)

func TestRubixCapture(t *testing.T) {
	dir := t.TempDir()
	s := &service{serviceAccountKeyFile: filepath.Join(dir, "sa.key"), nodeIP: "192.0.2.8", adminKubeconfig: filepath.Join(dir, "admin.config"), adminCertFile: filepath.Join(dir, "admin.crt"), adminKeyFile: filepath.Join(dir, "admin.key"), caFile: filepath.Join(dir, "ca.crt")}
	must := func(err error) {
		t.Helper()
		if err != nil {
			t.Fatal(err)
		}
	}
	must(s.generateServiceAccountKey())
	raw, err := os.ReadFile(s.serviceAccountKeyFile)
	must(err)
	block, _ := pem.Decode(raw)
	if block == nil {
		t.Fatal("missing PEM")
	}
	key, err := x509.ParsePKCS1PrivateKey(block.Bytes)
	must(err)
	must(key.Validate())
	info, err := os.Stat(s.serviceAccountKeyFile)
	must(err)
	must(s.generateServiceAccountKey())
	again, err := os.ReadFile(s.serviceAccountKeyFile)
	must(err)
	checks := map[string]any{"key_type": block.Type, "key_bits": key.N.BitLen(), "key_valid": true, "key_mode": fmt.Sprintf("%04o", info.Mode().Perm()), "restart_preserves_key": bytes.Equal(raw, again)}
	must(os.WriteFile(s.serviceAccountKeyFile, []byte("corrupt-synthetic"), 0600))
	checks["existing_corrupt_key_accepted"] = s.generateServiceAccountKey() == nil
	after, err := os.ReadFile(s.serviceAccountKeyFile)
	must(err)
	checks["existing_corrupt_key_preserved"] = string(after) == "corrupt-synthetic"
	s.serviceAccountKeyFile = filepath.Join(dir, "missing", "sa.key")
	checks["missing_parent_fails"] = s.generateServiceAccountKey() != nil
	checks["missing_certificate_fails"] = s.generateKubeConfig() != nil
	for path, data := range map[string]string{s.adminCertFile: "SYNTHETIC-CERT", s.adminKeyFile: "SYNTHETIC-KEY", s.caFile: "SYNTHETIC-CA"} {
		must(os.WriteFile(path, []byte(data), 0600))
	}
	must(s.generateKubeConfig())
	config, err := clientcmd.LoadFromFile(s.adminKubeconfig)
	must(err)
	info, err = os.Stat(s.adminKubeconfig)
	must(err)
	checks["kubeconfig_mode"] = fmt.Sprintf("%04o", info.Mode().Perm())
	first, err := os.ReadFile(s.adminKubeconfig)
	must(err)
	must(s.generateKubeConfig())
	second, err := os.ReadFile(s.adminKubeconfig)
	must(err)
	checks["kubeconfig_repeat_equal"] = bytes.Equal(first, second)
	must(os.WriteFile(s.adminCertFile, []byte("SYNTHETIC-REPLACED-CERT"), 0600))
	must(s.generateKubeConfig())
	updated, err := clientcmd.LoadFromFile(s.adminKubeconfig)
	must(err)
	checks["kubeconfig_refreshes_certificate"] = string(updated.AuthInfos["kubernetes-admin"].ClientCertificateData) == "SYNTHETIC-REPLACED-CERT"
	s.adminKubeconfig = dir
	checks["output_directory_fails"] = s.generateKubeConfig() != nil
	s.adminKubeconfig = filepath.Join(dir, "admin.config")
	s.adminKeyFile = dir
	checks["unreadable_key_fails"] = s.generateKubeConfig() != nil
	checks["baseline_static_token_matches"] = config.AuthInfos["admin-token"].Token == "admin-token"
	config.AuthInfos["admin-token"].Token = ""
	// Deliberately synthetic copy fixtures; actual signing key never leaves this process.
	value := map[string]any{"component": "apiserver", "synthetic_kubeconfig": config, "checks": checks}
	encoded, err := json.Marshal(value)
	must(err)
	fmt.Printf("RUBIX_CAPTURE %s\n", encoded)
}
