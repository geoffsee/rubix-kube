package embedded
import("bytes";"encoding/json";"fmt";"os";"path/filepath";"strings";"testing";"github.com/rs/zerolog";"github.com/rs/zerolog/log";"github.com/portainer/kubesolo/types")
func TestCapture(t *testing.T){
 var buf bytes.Buffer;log.Logger=zerolog.New(&buf);rows:=[]string{"owned\t"+types.DefaultCNIConfigName,"plugins\t"+strings.Join(requiredCNIPlugins,",")}
 // No /opt write: runtime image does not contain default plugin paths.
 warnMissingCNIPlugins();rows=append(rows,fmt.Sprintf("default_missing_warning\t%t",strings.Contains(buf.String(),"host has to provide")));buf.Reset()
 root:=t.TempDir();for _,name:=range []string{"00-first.conf","05-first.conflist","07-first.json",types.DefaultCNIConfigName,"20-last.conf","00-ignore.txt"}{if e:=os.WriteFile(filepath.Join(root,name),[]byte("untouched"),0444);e!=nil{t.Fatal(e)}}
 logCNIConfigOrder(filepath.Join(root,types.DefaultCNIConfigName));earlier,later:=0,0;for _,line:=range strings.Split(buf.String(),"\n"){var r map[string]any;if line==""{continue};if e:=json.Unmarshal([]byte(line),&r);e!=nil{t.Fatal(e)};if r["level"]=="warn"{earlier++}else if r["level"]=="info"{later++}}
 rows=append(rows,fmt.Sprintf("ordering\t%d,%d",earlier,later));for _,name:=range []string{"00-first.conf",types.DefaultCNIConfigName}{b,e:=os.ReadFile(filepath.Join(root,name));if e!=nil||string(b)!="untouched"{t.Fatal("mutated")}};rows=append(rows,"files_unchanged\ttrue")
 data,e:=json.Marshal(rows);if e!=nil{t.Fatal(e)};fmt.Printf("RUBIX_CAPTURE %s\n",data)
}
