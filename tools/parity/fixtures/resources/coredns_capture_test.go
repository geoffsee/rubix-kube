package coredns
import("context";"errors"; "k8s.io/apimachinery/pkg/runtime"; kt "k8s.io/client-go/testing";"encoding/json";"fmt";"testing"; metav1 "k8s.io/apimachinery/pkg/apis/meta/v1"; "k8s.io/client-go/kubernetes/fake"; )
func TestRubixCapture(t *testing.T) {
ctx:=context.Background(); _=ctx
for _,variant:=range []string{"host-dual","container-dual","host-ipv4","container-ipv4"} {
c:=fake.NewSimpleClientset(); _=c
for repeat:=0;repeat<2;repeat++ {
if err:=createServiceAccount(ctx,c);err!=nil{t.Fatal(err)}
if err:=createClusterRole(ctx,c);err!=nil{t.Fatal(err)}
if err:=createClusterRoleBinding(ctx,c);err!=nil{t.Fatal(err)}
if err:=createConfigMap(ctx,c,variant=="container-dual"||variant=="container-ipv4",variant=="host-ipv4"||variant=="container-ipv4");err!=nil{t.Fatal(err)}
if err:=createService(ctx,c);err!=nil{t.Fatal(err)}
if err:=createDeployment(ctx,c,variant=="container-dual"||variant=="container-ipv4");err!=nil{t.Fatal(err)}
}
bad:=fake.NewSimpleClientset();bad.PrependReactor("*","*",func(kt.Action)(bool,runtime.Object,error){return true,nil,errors.New("fixture injected failure")});if err:=createServiceAccount(ctx,bad);err==nil{t.Fatal("injected failure lost")}
checks:=map[string]bool{"repeat_success":true,"injected_error_propagated":true}
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
b,err:=json.Marshal(map[string]interface{}{"component":"coredns","variant":variant,"objects":objects,"checks":checks});if err!=nil{t.Fatal(err)};fmt.Printf("RUBIX_CAPTURE %s\n",b)
}}
