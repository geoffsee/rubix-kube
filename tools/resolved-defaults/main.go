// Development-only invocation of official API-server option completion.
package main

import (
	"context"
	"encoding/json"
	"fmt"
	"github.com/spf13/pflag"
	options "k8s.io/kubernetes/cmd/kube-apiserver/app/options"
	"net"
	"runtime"
	"sort"
	"time"
)

func run(name string) map[string]interface{} {
	o := options.NewServerRunOptions()
	// ExternalAddress makes DefaultAdvertiseAddress independent of host routing.
	o.SecureServing.BindAddress = net.ParseIP("127.0.0.1")
	o.SecureServing.ExternalAddress = net.ParseIP("127.0.0.1")
	o.SecureServing.ServerCert.CertDirectory = ""
	switch name {
	case "dual_stack":
		o.ServiceClusterIPRanges = "10.96.0.0/20,fd00:1234::/108"
		o.Authorization.Modes = []string{"Node", "RBAC"}
		o.GenericServerRunOptions.RequestTimeout = 120 * time.Second
		o.APIEnablement.RuntimeConfig = map[string]string{"api/v1": "true", "api/legacy": "false", "apps/v1": "true"}
		o.Etcd.WatchCacheSizes = []string{"pods#42"}
		o.Authentication.ServiceAccounts.MaxExpiration = 2 * time.Hour
	case "invalid_cidr":
		o.ServiceClusterIPRanges = "not-a-cidr"
	case "small_cidr":
		o.ServiceClusterIPRanges = "10.96.0.0/30"
	case "invalid_watch_cache":
		o.Etcd.WatchCacheSizes = []string{"pods#invalid"}
	case "invalid_token_expiration":
		o.Authentication.ServiceAccounts.MaxExpiration = 59 * time.Minute
	}
	before := map[string]string{}
	sets := o.Flags()
	for _, set := range sets.FlagSets {
		set.VisitAll(func(f *pflag.Flag) { before[f.Name] = f.Value.String() })
	}
	c, err := o.Complete(context.Background())
	if err != nil {
		return map[string]interface{}{"error": err.Error()}
	}
	// Watch-cache completion iterates a Go map: canonicalize this set before flag serialization.
	sort.Strings(c.Etcd.WatchCacheSizes)
	after := map[string]string{}
	for _, set := range sets.FlagSets {
		set.VisitAll(func(f *pflag.Flag) { after[f.Name] = f.Value.String() })
	}
	cache := append([]string{}, c.Etcd.WatchCacheSizes...)
	sort.Strings(cache)
	return map[string]interface{}{
		"flags_before": before, "flags_after": after,
		"primary_service_cidr":   c.PrimaryServiceClusterIPRange.String(),
		"secondary_service_cidr": c.SecondaryServiceClusterIPRange.String(),
		"service_ip":             c.APIServerServiceIP.String(),
		"advertise_address":      c.GenericServerRunOptions.AdvertiseAddress.String(),
		"external_host":          c.GenericServerRunOptions.ExternalHost,
		"authorization_modes":    c.Authorization.Modes, "anonymous_auth": c.Authentication.Anonymous.Allow,
		"watch_cache_sizes": cache, "events_history_window": c.Etcd.StorageConfig.EventsHistoryWindow.String(),
		"runtime_config":                c.APIEnablement.RuntimeConfig,
		"token_max_expiration":          c.ServiceAccountTokenMaxExpiration.String(),
		"generated_serving_certificate": c.SecureServing.ServerCert.GeneratedCert != nil,
		"serving_cert_file":             c.SecureServing.ServerCert.CertKey.CertFile,
		"serving_key_file":              c.SecureServing.ServerCert.CertKey.KeyFile,
		"listener_created":              c.SecureServing.Listener != nil,
	}
}
func main() {
	cases := map[string]interface{}{}
	for _, name := range []string{"default", "dual_stack", "invalid_cidr", "small_cidr", "invalid_watch_cache", "invalid_token_expiration"} {
		cases[name] = run(name)
	}
	output := map[string]interface{}{"schema_version": 1, "runtime": map[string]string{"go": runtime.Version(), "os": runtime.GOOS, "arch": runtime.GOARCH}, "cases": cases, "controls": map[string]interface{}{"bind_address": "127.0.0.1", "external_address": "127.0.0.1", "cert_directory": "", "operation": "ServerRunOptions.Complete", "server_started": false}}
	data, err := json.Marshal(output)
	if err != nil {
		panic(err)
	}
	fmt.Println("RUBIX_RESOLVED=" + string(data))
}
