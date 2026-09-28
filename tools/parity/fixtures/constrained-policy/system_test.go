package system
import("encoding/json";"fmt";"os";"path/filepath";"testing")
func TestMain(m *testing.M){if len(os.Args)>1&&os.Args[1]=="--version"{fmt.Print(os.Getenv("VERSION"));if os.Getenv("FAIL")=="1"{os.Exit(1)};os.Exit(0)};os.Exit(m.Run())}
func TestCapture(t *testing.T){
 root:=t.TempDir();binary,e:=os.Executable();if e!=nil{t.Fatal(e)};if e=os.Symlink(binary,filepath.Join(root,"iptables"));e!=nil{t.Fatal(e)};t.Setenv("PATH",root)
 rows:=[]string{};for _,c:=range []struct{name,version,fail string}{{"nft","iptables v1 (nf_tables)",""},{"legacy","iptables v1 (legacy)",""},{"unrecognized","garbage",""},{"failed_assumes_nft","","1"}}{t.Setenv("VERSION",c.version);t.Setenv("FAIL",c.fail);rows=append(rows,fmt.Sprintf("%s\t%t",c.name,isNftBackend()))}
 data,e:=json.Marshal(rows);if e!=nil{t.Fatal(e)};fmt.Printf("RUBIX_CAPTURE %s\n",data)
}
