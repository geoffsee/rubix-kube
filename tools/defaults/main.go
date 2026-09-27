// Development-only extraction from the pinned official Kubernetes source tree.
package main

import (
	"encoding/json"
	"fmt"
	"k8s.io/apimachinery/pkg/util/version"
	utilfeature "k8s.io/apiserver/pkg/util/feature"
	controller "k8s.io/kube-controller-manager/config/v1alpha1"
	proxy "k8s.io/kube-proxy/config/v1alpha1"
	kubelet "k8s.io/kubelet/config/v1beta1"
	controllerdefaults "k8s.io/kubernetes/pkg/controller/apis/config/v1alpha1"
	_ "k8s.io/kubernetes/pkg/features"
	kubeletdefaults "k8s.io/kubernetes/pkg/kubelet/apis/config/v1beta1"
	proxydefaults "k8s.io/kubernetes/pkg/proxy/apis/config/v1alpha1"
	"os"
	"runtime"
)

func main() {
	gate := utilfeature.DefaultMutableFeatureGate
	if err := gate.SetEmulationVersionAndMinCompatibilityVersion(version.MustParse("1.35"), version.MustParse("1.34")); err != nil {
		panic(err)
	}
	cases := map[string]interface{}{}
	for _, explicit := range []bool{false, true} {
		k := &kubelet.KubeletConfiguration{}
		p := &proxy.KubeProxyConfiguration{}
		c := &controller.KubeControllerManagerConfiguration{}
		name := "zero"
		if explicit {
			name = "explicit"
			k.Port = 10260
			k.ReadOnlyPort = 1234
			k.EnableServer = new(bool)
			p.BindAddress = "192.0.2.9"
			p.ClientConnection.QPS = 17
			c.Generic.ClientConnection.QPS = 19
			c.Generic.Controllers = []string{"fixture"}
		}
		kubeletdefaults.SetDefaults_KubeletConfiguration(k)
		proxydefaults.SetDefaults_KubeProxyConfiguration(p)
		controllerdefaults.SetDefaults_KubeControllerManagerConfiguration(c)
		cases[name] = map[string]interface{}{"kubelet": k, "proxy": p, "controller": c}
	}
	gates := map[string]interface{}{}
	for name, specs := range gate.GetAllVersioned() {
		values := []interface{}{}
		for _, spec := range specs {
			minimum := ""
			if spec.MinCompatibilityVersion != nil {
				minimum = spec.MinCompatibilityVersion.String()
			}
			values = append(values, map[string]interface{}{"default": spec.Default, "locked": spec.LockToDefault, "stage": spec.PreRelease, "version": spec.Version.String(), "minimum_compatibility": minimum})
		}
		gates[string(name)] = map[string]interface{}{"specs": values, "enabled": gate.Enabled(name)}
	}
	out := map[string]interface{}{"schema_version": 1, "platform": runtime.GOOS + "/" + runtime.GOARCH, "go_version": runtime.Version(), "source_revision": "96cb9ab4201d88ce5e549fde047a686171838fdb", "emulation_version": gate.EmulationVersion().String(), "minimum_compatibility_version": gate.MinCompatibilityVersion().String(), "feature_overrides": map[string]bool{}, "cases": cases, "registered_feature_gates": gates}
	encoder := json.NewEncoder(os.Stdout)
	encoder.SetIndent("", "  ")
	if err := encoder.Encode(out); err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
