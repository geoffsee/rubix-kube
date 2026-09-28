package detect

import (
 "encoding/json"
 "fmt"
 "os"
 "os/exec"
 "path/filepath"
 "strings"
 "syscall"
 "testing"
)

type fixture struct { Name string; Paths, Directories []string; Cgroup, Model string }
var fixtures=[]fixture{
 {Name:"empty"},
 {Name:"systemd_socket",Paths:[]string{"/run/systemd/private"}},
 {Name:"systemd_units",Paths:[]string{"/etc/systemd/system","/usr/bin/systemctl"}},
 {Name:"command_directory",Paths:[]string{"/etc/systemd/system"},Directories:[]string{"/usr/bin/systemctl"}},
 {Name:"custom_path_only",Paths:[]string{"/etc/systemd/system","/custom/bin/systemctl"}},
 {Name:"upstart",Paths:[]string{"/etc/init","/sbin/initctl"}},
 {Name:"openrc",Paths:[]string{"/sbin/openrc"}},
 {Name:"s6",Paths:[]string{"/etc/s6"}},
 {Name:"runit",Paths:[]string{"/var/service"}},
 {Name:"sysv",Paths:[]string{"/etc/init.d"}},
 {Name:"precedence",Paths:[]string{"/run/systemd/private","/sbin/openrc","/etc/s6","/etc/init.d"}},
 {Name:"docker_marker",Paths:[]string{"/.dockerenv"}},
 {Name:"podman_marker",Paths:[]string{"/run/.containerenv"}},
 {Name:"docker_cgroup",Cgroup:"0::/docker/fixture"},
 {Name:"embedded",Model:"Raspberry Pi fixture"},
 {Name:"musl",Paths:[]string{"/lib/ld-musl-aarch64.so.1"}},
}
func TestRubixChild(t *testing.T) {
 root:=os.Getenv("RUBIX_ROOT");if root==""{t.Skip("owned child only")}
 if err:=syscall.Chroot(root);err!=nil{t.Fatal(err)};if err:=os.Chdir("/");err!=nil{t.Fatal(err)}
 libc,_,err:=detectLibC("arm64");if err!=nil{t.Fatal(err)}
 fmt.Printf("RUBIX_ROW %s\t%s\t%s\t%s\n",os.Getenv("RUBIX_CASE"),detectInitSystem(),detectEnvironment(),libc)
}
func TestRubixCapture(t *testing.T) {
 var rows []string
 for _,f:=range fixtures{
  root:=t.TempDir()
  write:=func(path,value string){if err:=os.MkdirAll(filepath.Dir(root+path),0755);err!=nil{t.Fatal(err)};if err:=os.WriteFile(root+path,[]byte(value),0644);err!=nil{t.Fatal(err)}}
  for _,path:=range f.Paths{write(path,"")}
  for _,path:=range f.Directories{if err:=os.MkdirAll(root+path,0755);err!=nil{t.Fatal(err)}}
  if f.Cgroup!=""{write("/proc/1/cgroup",f.Cgroup)};if f.Model!=""{write("/proc/device-tree/model",f.Model)}
  command:=exec.Command(os.Args[0],"-test.run=^TestRubixChild$","-test.v");command.Env=[]string{"RUBIX_ROOT="+root,"RUBIX_CASE="+f.Name,"GOMAXPROCS=2","PATH=/custom/bin"}
  output,err:=command.CombinedOutput();if err!=nil{t.Fatalf("child %s: %v: %s",f.Name,err,output)}
  count:=0;for _,line:=range strings.Split(string(output),"\n"){if strings.HasPrefix(line,"RUBIX_ROW "){rows=append(rows,strings.TrimPrefix(line,"RUBIX_ROW "));count++}};if count!=1{t.Fatal("missing child observation")}
 }
 for _,target:=range []string{"amd64","arm64","arm","riscv64","amd64-musl","arm64-musl","arm-musl","riscv64-musl","mips"}{_,err:=ForTarget(target);rows=append(rows,fmt.Sprintf("target\t%s\t%t",target,err==nil))}
 encoded,err:=json.Marshal(rows);if err!=nil{t.Fatal(err)};fmt.Printf("RUBIX_CAPTURE %s\n",encoded)
}
