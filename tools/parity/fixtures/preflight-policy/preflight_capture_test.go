package preflight

import (
 "context"
 "encoding/json"
 "fmt"
 "net"
 "os"
 "os/exec"
 "path/filepath"
 "strings"
 "syscall"
 "testing"
 "time"
)
type fixture struct {name, check string; files map[string]string}
var cases=[]fixture{
 {"docker_absent","docker",nil},
 {"docker_socket","docker",map[string]string{"/var/run/docker.sock":""}},
 {"docker_binary","docker",map[string]string{"/usr/local/bin/docker":""}},
 {"comment_absent","comment",nil},
 {"comment_loaded","comment",map[string]string{"/proc/modules":"xt_comment\n"}},
 {"comment_match","comment",map[string]string{"/proc/net/ip_tables_matches":" tcp\n comment \n"}},
 {"comment_substring","comment",map[string]string{"/proc/net/ip_tables_matches":"not_comment\n"}},
 {"comment_disk","comment",map[string]string{"/proc/version":"Linux version fixture build", "/lib/modules/fixture/kernel/net/netfilter/xt_comment.ko.zst":""}},
 {"comment_glob","comment",map[string]string{"/proc/version":"Linux version fixture build", "/lib/modules/fixture/custom/xt_comment.ko.xz":""}},
 {"not_alpine","network",nil},
 {"alpine_missing","network",map[string]string{"/etc/alpine-release":""}},
 {"alpine_tools","network",map[string]string{"/etc/alpine-release":"","/usr/sbin/nft":"","/bin/iptables":""}},
 {"alpine_custom_paths","network",map[string]string{"/etc/alpine-release":"","/usr/local/bin/nft":"","/usr/local/bin/iptables":""}},
 {"v2_all","cgroups",map[string]string{"/sys/fs/cgroup/cgroup.controllers":"cpuset cpu io memory pids"}},
 {"v2_missing","cgroups",map[string]string{"/sys/fs/cgroup/cgroup.controllers":"cpu memory"}},
 {"v2_empty","cgroups",map[string]string{"/sys/fs/cgroup/cgroup.controllers":""}},
 {"v1_all","cgroups",map[string]string{"/sys/fs/cgroup/cpuset":"","/sys/fs/cgroup/cpu":"","/sys/fs/cgroup/blkio":"","/sys/fs/cgroup/memory":"","/sys/fs/cgroup/pids":""}},
 {"v1_io_wrong","cgroups",map[string]string{"/sys/fs/cgroup/cpuset":"","/sys/fs/cgroup/cpu":"","/sys/fs/cgroup/io":"","/sys/fs/cgroup/memory":"","/sys/fs/cgroup/pids":""}},
 {"alpine_rc_service","cgroups",map[string]string{"/etc/alpine-release":"","/sbin/rc-service":"","/sys/fs/cgroup/cgroup.controllers":""}},
 {"alpine_ready","cgroups",map[string]string{"/etc/alpine-release":"","/sbin/rc-service":"","/sys/fs/cgroup/cgroup.controllers":"cpuset cpu io memory pids"}},
}
func TestRubixChild(t *testing.T) {
 root:=os.Getenv("RUBIX_ROOT");if root==""{t.Skip("owned child only")}
 if err:=syscall.Chroot(root);err!=nil{t.Fatal(err)};if err:=os.Chdir("/");err!=nil{t.Fatal(err)}
 var err error
 switch os.Getenv("RUBIX_CHECK") {
 case "docker":err=CheckDockerConflict()
 case "comment":err=CheckIptablesComment()
 case "network":err=CheckAlpineNetworking(false)
 case "cgroups":err=CheckCgroups(false)
 default:t.Fatal("unknown check")
 }
 fmt.Printf("RUBIX_ROW %s\t%t\n",os.Getenv("RUBIX_CASE"),err==nil)
}
func TestRubixCapture(t *testing.T) {
 var rows []string
 rows=append(rows,fmt.Sprintf("root\t%t",CheckRoot()==nil))
 hostnames:=[]struct{name,value string}{
  {"hostname_valid","node-1.example"},{"hostname_uppercase","Node"},{"hostname_empty",""},
  {"hostname_dot","node..host"},{"hostname_underscore","node_host"},{"hostname_space"," node"},
  {"hostname_label63",strings.Repeat("a",63)},{"hostname_label64",strings.Repeat("a",64)},
  {"hostname_total253",strings.Repeat("a",63)+"."+strings.Repeat("b",63)+"."+strings.Repeat("c",63)+"."+strings.Repeat("d",61)},
  {"hostname_total254",strings.Repeat("a",63)+"."+strings.Repeat("b",63)+"."+strings.Repeat("c",63)+"."+strings.Repeat("d",62)},
  {"hostname_unicode","nœud"},{"hostname_dash","-node"},
 }
 for _,h:=range hostnames {rows=append(rows,fmt.Sprintf("%s\t%t",h.name,validateHostname(h.value)==nil))}
 for _,f:=range cases {
  root:=t.TempDir()
  for path,value:=range f.files {if err:=os.MkdirAll(filepath.Dir(root+path),0755);err!=nil{t.Fatal(err)};if err:=os.WriteFile(root+path,[]byte(value),0644);err!=nil{t.Fatal(err)}}
  ctx,cancel:=context.WithTimeout(context.Background(),5*time.Second)
  cmd:=exec.CommandContext(ctx,os.Args[0],"-test.run=^TestRubixChild$","-test.v");cmd.Env=[]string{"RUBIX_ROOT="+root,"RUBIX_CHECK="+f.check,"RUBIX_CASE="+f.name,"GOMAXPROCS=2"}
  output,err:=cmd.CombinedOutput();cancel();if err!=nil{t.Fatalf("child %s: %v %s",f.name,err,output)}
  found:=0;for _,line:=range strings.Split(string(output),"\n"){if strings.HasPrefix(line,"RUBIX_ROW "){rows=append(rows,strings.TrimPrefix(line,"RUBIX_ROW "));found++}};if found!=1{t.Fatal("child record count")}
 }
 for _,port:=range []int{0,6060,2379,6443,10443} {
  var listener net.Listener
  if port!=0 {var err error;listener,err=net.Listen("tcp",fmt.Sprintf(":%d",port));if err!=nil{t.Fatal(err)}}
  for _,pprof:=range []bool{false,true}{rows=append(rows,fmt.Sprintf("ports_%d_%t\t%t",port,pprof,CheckPorts(pprof)==nil))}
  if listener!=nil{if err:=listener.Close();err!=nil{t.Fatal(err)}}
 }
 invoked:=0
 err:=RunSuite([]Check{{Name:"fail",Run:func()error{return fmt.Errorf("synthetic failure")}}, {Name:"must_not_run",Run:func()error{invoked++;return nil}}})
 rows=append(rows,fmt.Sprintf("suite_failfast\t%t",err!=nil&&invoked==0))
 encoded,err:=json.Marshal(rows);if err!=nil{t.Fatal(err)};fmt.Printf("RUBIX_CAPTURE %s\n",encoded)
}
