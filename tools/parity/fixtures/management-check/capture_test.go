package cli
import("bytes";"context";"encoding/json";"fmt";"os";"os/exec";"testing";"time";"github.com/spf13/cobra";"github.com/portainer/kubesolo/internal/cli/config")
// Sibling commands are inert registrations; their implementation/help is not qualified.
func sibling(name string)*cobra.Command{return &cobra.Command{Use:name}}
func installCmd(*config.Config)*cobra.Command{return sibling("install")}
func uninstallCmd()*cobra.Command{return sibling("uninstall")}
func upgradeCmd(*config.Config)*cobra.Command{return sibling("upgrade")}
func kubeconfigCmd()*cobra.Command{return sibling("kubeconfig")}
func configCmd()*cobra.Command{return sibling("config")}
func d2kCmd()*cobra.Command{return sibling("d2k")}
func downloadCmd(*config.Config)*cobra.Command{return sibling("download")}
func resetCmd()*cobra.Command{return sibling("reset")}
func completionCmd()*cobra.Command{return sibling("completion")}
// Explicit injected effect boundary: actual command parser, no host checks/preparation.
func runCheck(cfg *config.Config)error{fmt.Printf("CHECK %t %t\n",cfg.InstallPrereqs,cfg.PprofServer);return nil}
type Case struct{Name string `json:"name"`;Args []string `json:"args"`;Env map[string]string `json:"env"`}
func TestMain(m *testing.M){if os.Getenv("RUBIX_CHILD")=="1"{err:=rootCmd().Execute();if err!=nil{fmt.Fprintln(os.Stderr,"error:",err);os.Exit(1)};os.Exit(0)};os.Exit(m.Run())}
func TestCapture(t *testing.T){data,e:=os.ReadFile("/cases.json");if e!=nil{t.Fatal(e)};var cases []Case;if e=json.Unmarshal(data,&cases);e!=nil{t.Fatal(e)};rows:=[]map[string]any{}
 for _,c:=range cases{ctx,cancel:=context.WithTimeout(context.Background(),3*time.Second);cmd:=exec.CommandContext(ctx,os.Args[0],c.Args...);cmd.Env=[]string{"RUBIX_CHILD=1","NO_COLOR=1","TERM=dumb","GOMAXPROCS=2"};for k,v:=range c.Env{cmd.Env=append(cmd.Env,k+"="+v)};var out,err bytes.Buffer;cmd.Stdout=&out;cmd.Stderr=&err;e:=cmd.Run();cancel();code:=0;if e!=nil{exit,ok:=e.(*exec.ExitError);if !ok{t.Fatal(e)};code=exit.ExitCode()};rows=append(rows,map[string]any{"name":c.Name,"stdout":out.String(),"stderr":err.String(),"exit":code})
 };encoded,e:=json.Marshal(rows);if e!=nil{t.Fatal(e)};fmt.Printf("RUBIX_CAPTURE %s\n",encoded)
}
