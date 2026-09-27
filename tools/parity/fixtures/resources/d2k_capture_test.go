package d2k
import("context";"os";"path/filepath";"github.com/portainer/kubesolo/types";"errors"; "k8s.io/apimachinery/pkg/runtime"; kt "k8s.io/client-go/testing";"encoding/json";"fmt";"testing"; metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"; "k8s.io/client-go/kubernetes/fake"; )
func TestRubixCapture(t *testing.T) {
ctx:=context.Background(); _=ctx
for _,variant:=range []string{"custom-namespace"} {
c:=fake.NewSimpleClientset(); _=c
for repeat:=0;repeat<2;repeat++ {
if err:=createNamespace(ctx,c,"fixture-d2k");err!=nil{t.Fatal(err)}
if err:=createServiceAccount(ctx,c,"fixture-d2k");err!=nil{t.Fatal(err)}
if err:=createRole(ctx,c,"fixture-d2k");err!=nil{t.Fatal(err)}
if err:=createRoleBinding(ctx,c,"fixture-d2k");err!=nil{t.Fatal(err)}
if err:=createNodeReaderClusterRole(ctx,c);err!=nil{t.Fatal(err)}
if err:=createNodeReaderClusterRoleBinding(ctx,c,"fixture-d2k");err!=nil{t.Fatal(err)}
if err:=createDeployment(ctx,c,"fixture-d2k","fixture/d2k:fixed");err!=nil{t.Fatal(err)}
if err:=createService(ctx,c,"fixture-d2k");err!=nil{t.Fatal(err)}
}
bad:=fake.NewSimpleClientset();bad.PrependReactor("*","*",func(kt.Action)(bool,runtime.Object,error){return true,nil,errors.New("fixture injected failure")});if err:=createNamespace(ctx,bad,"fixture-d2k");err==nil{t.Fatal("injected failure lost")}
checks:=map[string]bool{"repeat_success":true,"injected_error_propagated":true}

 dir:=t.TempDir();certs:=types.D2KCertificatePaths{ServerCert:filepath.Join(dir,"tls.crt"),ServerKey:filepath.Join(dir,"tls.key")}
 checks["missing_tls_input_rejected"]=createTLSSecret(ctx,c,"fixture-d2k",certs)!=nil
 for path,data:=range map[string]string{certs.ServerCert:"SYNTHETIC PUBLIC CERT",certs.ServerKey:"SYNTHETIC KEY NOT SECRET"}{if err:=os.WriteFile(path,[]byte(data),0600);err!=nil{t.Fatal(err)}}
 if err:=createTLSSecret(ctx,c,"fixture-d2k",certs);err!=nil{t.Fatal(err)}
 if err:=os.WriteFile(certs.ServerCert,[]byte("ROTATED SYNTHETIC CERT"),0600);err!=nil{t.Fatal(err)}
 if err:=createTLSSecret(ctx,c,"fixture-d2k",certs);err!=nil{t.Fatal(err)}
 secret,err:=c.CoreV1().Secrets("fixture-d2k").Get(ctx,TLSSecretName,metav1.GetOptions{});if err!=nil{t.Fatal(err)}
 checks["tls_secret_updated"]=string(secret.Data["tls.crt"])=="ROTATED SYNTHETIC CERT"

objects:=map[string]interface{}{}
{v,e:=c.CoreV1().Namespaces().List(ctx,metav1.ListOptions{});if e!=nil{t.Fatal(e)};objects["namespaces"]=v.Items}
{v,e:=c.CoreV1().ServiceAccounts("").List(ctx,metav1.ListOptions{});if e!=nil{t.Fatal(e)};objects["serviceaccounts"]=v.Items}
{v,e:=c.CoreV1().ConfigMaps("").List(ctx,metav1.ListOptions{});if e!=nil{t.Fatal(e)};objects["configmaps"]=v.Items}
{v,e:=c.CoreV1().Secrets("").List(ctx,metav1.ListOptions{});if e!=nil{t.Fatal(e)};objects["secrets"]=v.Items}
{v,e:=c.CoreV1().Services("").List(ctx,metav1.ListOptions{});if e!=nil{t.Fatal(e)};objects["services"]=v.Items}
{v,e:=c.AppsV1().Deployments("").List(ctx,metav1.ListOptions{});if e!=nil{t.Fatal(e)};objects["deployments"]=v.Items}
{v,e:=c.RbacV1().Roles("").List(ctx,metav1.ListOptions{});if e!=nil{t.Fatal(e)};objects["roles"]=v.Items}
{v,e:=c.RbacV1().RoleBindings("").List(ctx,metav1.ListOptions{});if e!=nil{t.Fatal(e)};objects["rolebindings"]=v.Items}
{v,e:=c.RbacV1().ClusterRoles().List(ctx,metav1.ListOptions{});if e!=nil{t.Fatal(e)};objects["clusterroles"]=v.Items}
{v,e:=c.RbacV1().ClusterRoleBindings().List(ctx,metav1.ListOptions{});if e!=nil{t.Fatal(e)};objects["clusterrolebindings"]=v.Items}
{v,e:=c.StorageV1().StorageClasses().List(ctx,metav1.ListOptions{});if e!=nil{t.Fatal(e)};objects["storageclasses"]=v.Items}
b,err:=json.Marshal(map[string]interface{}{"component":"d2k","variant":variant,"objects":objects,"checks":checks});if err!=nil{t.Fatal(err)};fmt.Printf("RUBIX_CAPTURE %s\n",b)
}}
