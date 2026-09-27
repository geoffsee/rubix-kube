//go:build rubix_apiserver

package main

import (
	"github.com/spf13/pflag"
	options "k8s.io/kubernetes/cmd/kube-apiserver/app/options"
)

func init() {
	additionalOptions = func() interface{} {
		opts := options.NewServerRunOptions()
		flags := map[string]interface{}{}
		sections := opts.Flags()
		for section, set := range sections.FlagSets {
			set.VisitAll(func(flag *pflag.Flag) {
				if _, exists := flags[flag.Name]; exists {
					panic("duplicate API-server flag: " + flag.Name)
				}
				flags[flag.Name] = map[string]interface{}{"section": section, "type": flag.Value.Type(), "default": flag.DefValue, "hidden": flag.Hidden, "deprecated": flag.Deprecated}
			})
		}
		return map[string]interface{}{"flags": flags, "system_namespaces": opts.SystemNamespaces, "kubelet_preferred_address_types": opts.KubeletConfig.PreferredAddressTypes, "service_node_port_range": opts.ServiceNodePortRange.String(), "construction_only": true}
	}
}
