package webhook

import (
	"bytes"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"

	corev1 "k8s.io/api/core/v1"
	metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"
	"k8s.io/apimachinery/pkg/runtime"
	"k8s.io/client-go/kubernetes/fake"
	kt "k8s.io/client-go/testing"
)

type failingBody struct{}

func (failingBody) Read([]byte) (int, error) { return 0, errors.New("synthetic read failure") }
func (failingBody) Close() error             { return nil }
func TestRubixCapture(t *testing.T) {
	must := func(e error) {
		t.Helper()
		if e != nil {
			t.Fatal(e)
		}
	}
	dir := t.TempDir()
	must(os.Mkdir(filepath.Join(dir, "webhook"), 0700))
	must(os.WriteFile(filepath.Join(dir, "webhook/webhook.crt"), []byte("SYNTHETIC-WEBHOOK-CERT"), 0600))
	service := NewService("fixture-node", "192.0.2.9", dir, "", true)
	configurations := map[string]any{}
	for _, enabled := range []bool{false, true} {
		service.loadBalancer = enabled
		config, e := service.createConfiguration()
		must(e)
		configurations[fmt.Sprint(enabled)] = config
	}
	service.loadBalancer = true
	missing := NewService("fixture-node", "192.0.2.9", filepath.Join(dir, "absent"), "", true)
	_, e := missing.createConfiguration()
	configurations["missing_certificate_fails"] = e != nil
	type requestCase struct {
		name, method, kind, operation, object, contentType string
		dry                                                bool
		body                                               io.ReadCloser
		raw                                                string
		disabled                                           bool
		noIP                                               bool
	}
	cases := []requestCase{
		{name: "pod_unassigned", kind: "Pod", object: `{"metadata":{"name":"pod"},"spec":{}}`},
		{name: "pod_assigned", kind: "Pod", object: `{"spec":{"nodeName":"other-node"}}`},
		{name: "pod_update_direct", kind: "Pod", operation: "UPDATE", object: `{"spec":{}}`},
		{name: "pvc_empty", kind: "PersistentVolumeClaim", object: `{"metadata":{}}`},
		{name: "pvc_existing_annotation", kind: "PersistentVolumeClaim", object: `{"metadata":{"annotations":{"keep":"value"}}}`},
		{name: "pvc_assigned", kind: "PersistentVolumeClaim", object: `{"metadata":{"annotations":{"volume.kubernetes.io/selected-node":"other-node"}}}`},
		{name: "job_empty", kind: "Job", object: `{"spec":{"template":{"spec":{}}}}`},
		{name: "job_existing_selector", kind: "Job", object: `{"spec":{"template":{"spec":{"nodeSelector":{"keep":"value"}}}}}`},
		{name: "unknown_kind", kind: "Unknown", object: `{}`},
		{name: "malformed_typed_object", kind: "Pod", object: `{"spec":"invalid"}`},
		{name: "service_dry_run", kind: "Service", object: `{"metadata":{"name":"svc","namespace":"default"},"spec":{"type":"LoadBalancer"}}`, dry: true},
		{name: "service_disabled", kind: "Service", object: `{"metadata":{"name":"svc","namespace":"default"},"spec":{"type":"LoadBalancer"}}`, disabled: true},
		{name: "service_no_address", kind: "Service", object: `{"metadata":{"name":"svc","namespace":"default"},"spec":{"type":"LoadBalancer"}}`, noIP: true},
		{name: "service_clusterip", kind: "Service", object: `{"metadata":{"name":"svc","namespace":"default"},"spec":{"type":"ClusterIP"}}`},
		{name: "wrong_content_type", kind: "Pod", object: `{"spec":{}}`, contentType: "text/plain"},
		{name: "wrong_method", method: "GET", raw: `{}`},
		{name: "malformed_body", raw: `{`},
		{name: "missing_request", raw: `{"apiVersion":"admission.k8s.io/v1","kind":"AdmissionReview"}`},
		{name: "read_failure", body: failingBody{}},
	}
	requests := map[string]any{}
	for _, c := range cases {
		method := c.method
		if method == "" {
			method = "POST"
		}
		operation := c.operation
		if operation == "" {
			operation = "CREATE"
		}
		raw := c.raw
		if raw == "" {
			raw = fmt.Sprintf(`{"apiVersion":"admission.k8s.io/v1","kind":"AdmissionReview","request":{"uid":"fixture-uid","kind":{"group":"","version":"v1","kind":%q},"resource":{"group":"","version":"v1","resource":"fixture"},"operation":%q,"dryRun":%t,"object":%s}}`, c.kind, operation, c.dry, c.object)
		}
		req := httptest.NewRequest(method, "https://fixture/mutate", bytes.NewBufferString(raw))
		if c.body != nil {
			req.Body = c.body
		}
		ctype := c.contentType
		if ctype == "" {
			ctype = "application/json"
		}
		req.Header.Set("Content-Type", ctype)
		recorder := httptest.NewRecorder()
		service.loadBalancer = !c.disabled
		service.loadBalancerIP = "192.0.2.9"
		if c.noIP {
			service.loadBalancerIP = ""
		}
		service.serveMutate(recorder, req)
		record := map[string]any{"status": recorder.Code, "content_type": recorder.Header().Get("Content-Type")}
		if recorder.Code == http.StatusOK {
			var value map[string]any
			must(json.Unmarshal(recorder.Body.Bytes(), &value))
			record["admission_review"] = value
		} else {
			record["error"] = recorder.Body.String()
		}
		locked := false
		service.loadBalancerUpdateLocks.Range(func(_, _ any) bool { locked = true; return false })
		record["scheduled_status_update"] = locked
		requests[c.name] = record
	}
	service.loadBalancerIP = "192.0.2.9"
	service.loadBalancer = true
	statuses := map[string]any{}
	for _, name := range []string{"assign", "already_correct", "stale_type", "get_failure", "patch_failure", "exhausted"} {
		object := &corev1.Service{ObjectMeta: metav1.ObjectMeta{Name: "svc", Namespace: "default"}, Spec: corev1.ServiceSpec{Type: corev1.ServiceTypeLoadBalancer}}
		if name == "already_correct" {
			object.Status.LoadBalancer.Ingress = []corev1.LoadBalancerIngress{{IP: "192.0.2.9"}}
		}
		client := fake.NewSimpleClientset(object)
		gets := 0
		patches := 0
		client.PrependReactor("get", "services", func(a kt.Action) (bool, runtime.Object, error) {
			gets++
			if name == "exhausted" || name == "get_failure" && gets == 1 {
				return true, nil, errors.New("synthetic get failure")
			}
			if name == "stale_type" && gets == 1 {
				stale := object.DeepCopy()
				stale.Spec.Type = corev1.ServiceTypeClusterIP
				return true, stale, nil
			}
			return false, nil, nil
		})
		client.PrependReactor("patch", "services", func(a kt.Action) (bool, runtime.Object, error) {
			patches++
			if name == "patch_failure" && patches == 1 {
				return true, nil, errors.New("synthetic patch failure")
			}
			return false, nil, nil
		})
		err := service.updateLoadBalancerStatusWithRetry(context.Background(), "default", "svc", client)
		actions := []any{}
		for _, a := range client.Actions() {
			entry := map[string]any{"verb": a.GetVerb(), "resource": a.GetResource().Resource, "namespace": a.GetNamespace(), "subresource": a.GetSubresource()}
			if p, ok := a.(kt.PatchAction); ok {
				var patch any
				must(json.Unmarshal(p.GetPatch(), &patch))
				entry["patch"] = patch
				entry["patch_type"] = string(p.GetPatchType())
				entry["name"] = p.GetName()
			}
			if g, ok := a.(kt.GetAction); ok {
				entry["name"] = g.GetName()
			}
			actions = append(actions, entry)
		}
		message := ""
		if err != nil {
			message = err.Error()
		}
		statuses[name] = map[string]any{"actions": actions, "error": message, "get_count": gets, "patch_count": patches}
	}
	out := map[string]any{"component": "webhook", "configurations": configurations, "handler_requests": requests, "fake_client_status": statuses}
	encoded, e := json.Marshal(out)
	must(e)
	fmt.Printf("RUBIX_CAPTURE %s\n", encoded)
}
