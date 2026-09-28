package preflight

import (
 "encoding/json"
 "fmt"
 "os"
 "testing"
)

func TestOwnedGuestPreparation(t *testing.T) {
 if os.Getenv("RUBIX_ALPINE_GUEST") != "1" { t.Fatal("owned guest opt-in required") }
 if _, err:=os.Stat("/etc/alpine-release");err!=nil {t.Fatal(err)}
 phase:=os.Getenv("RUBIX_ACTION")
 install:=os.Getenv("RUBIX_INSTALL")=="1"
 var err error
 switch phase {case "network":err=CheckAlpineNetworking(install);case "cgroups":err=CheckCgroups(install);default:t.Fatal("unknown action")}
 outcome:="";if err!=nil {outcome=err.Error()}
 record:=map[string]any{"action":phase,"install":install,"error":outcome}
 encoded,e:=json.Marshal(record);if e!=nil {t.Fatal(e)}
 fmt.Printf("RUBIX_ALPINE_BASELINE %s\n",encoded)
}
