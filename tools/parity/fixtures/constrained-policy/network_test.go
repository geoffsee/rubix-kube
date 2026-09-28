package network
import("context";"encoding/json";"fmt";"os";"os/exec";"path/filepath";"strings";"syscall";"testing";"time")
func TestChild(t *testing.T){
 root:=os.Getenv("ROOT");if root==""{t.Skip()};if e:=syscall.Chroot(root);e!=nil{t.Fatal(e)};if e:=os.Chdir("/");e!=nil{t.Fatal(e)};if e:=syscall.Setuid(65534);e!=nil{t.Fatal(e)}
 err:=DisableIPv6Sysctls();fmt.Printf("ROW %t\n",err==nil)
}
func TestCapture(t *testing.T){
 cases:=[]struct{name,value string;mode os.FileMode;absent bool}{
 {"correct_readonly","1\n",0444,false},{"incorrect_readonly","0\n",0444,false},{"absent","",0444,true},{"unreadable_skipped","0\n",0000,false},{"prefix_one_accepted","10garbage",0444,false},{"incorrect_writable","0\n",0666,false},
 };rows:=[]string{}
 for _,c:=range cases{root,e:=os.MkdirTemp("/tmp","sysctl-");if e!=nil{t.Fatal(e)};if e=os.Chmod(root,0755);e!=nil{t.Fatal(e)}
  for _,path:=range ipv6SysctlPaths{if !c.absent{if e=os.MkdirAll(filepath.Dir(root+path),0755);e!=nil{t.Fatal(e)};if e=os.WriteFile(root+path,[]byte(c.value),c.mode);e!=nil{t.Fatal(e)};if e=os.Chmod(root+path,c.mode);e!=nil{t.Fatal(e)}}}
  ctx,cancel:=context.WithTimeout(context.Background(),5*time.Second);cmd:=exec.CommandContext(ctx,os.Args[0],"-test.run=^TestChild$","-test.v");cmd.Env=[]string{"ROOT="+root};out,e:=cmd.CombinedOutput();cancel();if e!=nil{t.Fatalf("%v %s",e,out)}
  found:=0;for _,line:=range strings.Split(string(out),"\n"){if strings.HasPrefix(line,"ROW "){rows=append(rows,c.name+"\t"+strings.TrimPrefix(line,"ROW "));found++}};if found!=1{t.Fatal("row count")}
  if !c.absent{if e=os.Chmod(root+ipv6SysctlPaths[0],0644);e!=nil{t.Fatal(e)};actual,e:=os.ReadFile(root+ipv6SysctlPaths[0]);if e!=nil{t.Fatal(e)};rows=append(rows,c.name+"_bytes\t"+fmt.Sprintf("%x",actual))};if e=os.RemoveAll(root);e!=nil{t.Fatal(e)}
 };data,e:=json.Marshal(rows);if e!=nil{t.Fatal(e)};fmt.Printf("RUBIX_CAPTURE %s\n",data)
}
