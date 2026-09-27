package configapi
import("bytes";"context";"encoding/json";"fmt";"net";"os";"path/filepath";"strings";"sync";"testing";"time";"github.com/portainer/kubesolo/types";"github.com/portainer/kubesolo/internal/config")
func TestRubixCapture(t *testing.T){
 client,path:=api(t,func(c *types.Config){c.Network.NodeIP="192.0.2.10";c.D2K.Namespace="workloads";c.Portainer.EdgeKey="fixture-synthetic-key"})
 t.Setenv("KUBESOLO_NODE_IP","192.0.2.99")
 effective,_,err:=config.Load(path,config.FlagValues{});if err!=nil{t.Fatal(err)}
 records:=map[string]interface{}{}
 request:=func(name,method,url,body string,headers map[string]string)string{resp,raw:=do(t,client,method,url,body,headers);var document interface{};if err:=json.Unmarshal(raw,&document);err!=nil{document=string(raw)};records[name]=map[string]interface{}{"status":resp.StatusCode,"body":document,"raw_body":string(raw),"etag":resp.Header.Get("ETag")};return resp.Header.Get("ETag")}
 initialTag:=request("get","GET","/api/v1/config","",nil)
 request("show-secrets","GET","/api/v1/config?showSecrets=true","",nil)
 request("schema","GET","/api/v1/config/schema","",nil)
 request("patch-current-etag","PATCH","/api/v1/config",`{"network":{"mtu":1400}}`,map[string]string{"If-Match":initialTag})
 request("patch-stale-etag","PATCH","/api/v1/config",`{"network":{"mtu":1500}}`,map[string]string{"If-Match":initialTag})
 request("patch-null","PATCH","/api/v1/config",`{"d2k":{"namespace":null}}`,nil)
 before,err:=os.ReadFile(path);if err!=nil{t.Fatal(err)}
 request("validate","POST","/api/v1/config:validate",`{"network":{"nodeIP":"192.0.2.30"}}`,nil)
 request("validate-invalid","POST","/api/v1/config:validate",`{"d2k":{"enabled":true},"network":{"loadBalancer":{"enabled":false}}}`,nil)
 after,err:=os.ReadFile(path);if err!=nil{t.Fatal(err)};checks:=map[string]bool{"validate_does_not_write":bytes.Equal(before,after)}
 for _,tc:=range []struct{name,method,body,content string}{
 {"immutable","PATCH",`{"path":"/elsewhere"}`,""},{"invalid","PATCH",`{"d2k":{"enabled":true},"network":{"loadBalancer":{"enabled":false}}}`,""},
 {"malformed-patch","PATCH","{not json",""},{"malformed-put","PUT","{not json",""},{"wrong-type","PUT",`{"network":{"mtu":"tall"}}`,""},{"wrong-content","PATCH","{}","application/xml"},
 {"redacted-write","PATCH",`{"portainer":{"edgeKey":"***"}}`,""},{"oversized","PATCH",strings.Repeat("x",maxBodySize+1),""},
 }{request(tc.name,tc.method,"/api/v1/config",tc.body,map[string]string{"Content-Type":tc.content})}
 after,err=os.ReadFile(path);if err!=nil{t.Fatal(err)};checks["rejected_requests_do_not_write"]=bytes.Equal(before,after)
 request("put-replacement","PUT","/api/v1/config",`{"network":{"mtu":1450}}`,nil)
 request("delete-defaults","DELETE","/api/v1/config","",nil)
 // Concurrent requests change disjoint fields through the real socket.
 var wg sync.WaitGroup;for _,patch:=range []string{`{"network":{"mtu":1400}}`,`{"logging":{"debug":true}}`}{wg.Add(1);go func(body string){defer wg.Done();resp,_:=do(t,client,"PATCH","/api/v1/config",body,nil);if resp.StatusCode!=200{t.Error("concurrent patch failed")}}(patch)};wg.Wait()
 request("after-concurrent-patches","GET","/api/v1/config","",nil)
 request("no-op-patch","PATCH","/api/v1/config",`{"network":{"mtu":1400}}`,nil)
 request("health","GET","/healthz","",nil)
 // Separate service allows explicit readiness, shutdown and restart observations.
 socket:=filepath.Join(shortTempDir(t),"s.sock");ctx,cancel:=context.WithCancel(context.Background());service:=NewService(ctx,cancel,make(chan struct{}),Options{SocketPath:socket});done:=make(chan error,1);go func(){done<-service.Run()}()
 select{case <-service.readyCh:case err:=<-done:t.Fatal(err);case <-time.After(5*time.Second):t.Fatal("readiness timeout")}
 stat,err:=os.Stat(socket);if err!=nil{t.Fatal(err)};socketMode:=fmt.Sprintf("%04o",stat.Mode().Perm());checks["live_socket_refused"]=service.clearStaleSocket()!=nil
 cancel();select{case err:=<-done:if err!=nil{t.Fatal(err)};case <-time.After(6*time.Second):t.Fatal("shutdown timeout")};_,err=os.Lstat(socket);checks["shutdown_removes_socket"]=os.IsNotExist(err)
 // Leave an actual Unix socket inode without a listener.
 listener,err:=net.ListenUnix("unix",&net.UnixAddr{Name:socket,Net:"unix"});if err!=nil{t.Fatal(err)};listener.SetUnlinkOnClose(false);if err=listener.Close();err!=nil{t.Fatal(err)}
 checks["stale_socket_reclaimed"]=service.clearStaleSocket()==nil;_,err=os.Lstat(socket);checks["stale_socket_removed"]=os.IsNotExist(err)
 if err=os.WriteFile(socket,[]byte("keep"),0600);err!=nil{t.Fatal(err)};checks["regular_file_refused"]=service.clearStaleSocket()!=nil;raw,err:=os.ReadFile(socket);if err!=nil{t.Fatal(err)};checks["regular_file_preserved"]=string(raw)=="keep"
 out:=map[string]interface{}{"requests":records,"checks":checks,"socket_mode":socketMode,"effective_node_ip":effective.Network.NodeIP};b,err:=json.Marshal(out);if err!=nil{t.Fatal(err)};fmt.Printf("RUBIX_CAPTURE %s\n",b)
}
