package config
import("bytes";"encoding/json";"fmt";"os";"path/filepath";"syscall";"testing")
func TestRubixCapture(t *testing.T){
 root:=t.TempDir();path:=filepath.Join(root,"nested","config.yaml");cfg:=Defaults();cfg.Portainer.EdgeKey="fixture-synthetic-key"
 if err:=os.MkdirAll(filepath.Dir(path),0700);err!=nil{t.Fatal(err)}
 originalMask:=syscall.Umask(0277);defer syscall.Umask(originalMask)
 if err:=Write(path,cfg);err!=nil{t.Fatal(err)};first,err:=os.ReadFile(path);if err!=nil{t.Fatal(err)};cfg.Network.MTU=1400
 if err=Write(path,cfg);err!=nil{t.Fatal(err)};second,err:=os.ReadFile(path);if err!=nil{t.Fatal(err)};backup,err:=os.ReadFile(path+".bak");if err!=nil{t.Fatal(err)}
 mode:=func(p string)string{s,e:=os.Stat(p);if e!=nil{t.Fatal(e)};return fmt.Sprintf("%04o",s.Mode().Perm())}
 out:=map[string]interface{}{"first":string(first),"second":string(second),"backup_matches_first":bytes.Equal(first,backup),"target_mode":mode(path),"backup_mode":mode(path+".bak")}
 // A backup-path directory forces failure after staging but before replacement.
 if err=os.Remove(path+".bak");err!=nil{t.Fatal(err)};if err=os.Mkdir(path+".bak",0700);err!=nil{t.Fatal(err)};cfg.Network.MTU=1450;writeErr:=Write(path,cfg);after,err:=os.ReadFile(path);if err!=nil{t.Fatal(err)}
 out["failed_write_rejected"]=writeErr!=nil;out["failed_write_preserves_original"]=bytes.Equal(second,after);entries,err:=os.ReadDir(filepath.Dir(path));if err!=nil{t.Fatal(err)};names:=[]string{};for _,entry:=range entries{names=append(names,entry.Name())};out["remaining_entries"]=names
 // Existing backup permissions are characterized separately from creation.
 if err=os.Remove(path+".bak");err!=nil{t.Fatal(err)};syscall.Umask(0);if err=os.WriteFile(path+".bak",[]byte("old"),0644);err!=nil{t.Fatal(err)};if err=Write(path,cfg);err!=nil{t.Fatal(err)};out["preexisting_backup_mode"]=mode(path+".bak")
 if err=os.Remove(path+".bak");err!=nil{t.Fatal(err)};if err=Write(path,cfg);err!=nil{t.Fatal(err)};out["ordinary_backup_mode"]=mode(path+".bak")
 b,err:=json.Marshal(out);if err!=nil{t.Fatal(err)};fmt.Printf("RUBIX_CAPTURE %s\n",b)
}
