package pki

import (
 "bytes"
 "crypto/rand"
 "crypto/rsa"
 "crypto/sha256"
 "crypto/x509"
 "encoding/json"
 "encoding/pem"
 "fmt"
 "os"
 "path/filepath"
 "testing"
 "time"
 "github.com/portainer/kubesolo/internal/config"
 "github.com/portainer/kubesolo/types"
)

var captureTypes=[]CertificateType{CACert,KubeletCert,APIServerCert,ControllerManagerCert,AdminCert,WebhookCert,RequestHeaderCACert,RequestHeaderClientCert,D2KServerCert,D2KClientCert}
func captureRead(t *testing.T,p string) []byte { t.Helper();b,e:=os.ReadFile(p);if e!=nil{t.Fatal(e)};return b }
func captureCert(t *testing.T,p string)*x509.Certificate{t.Helper();b,_:=pem.Decode(captureRead(t,p));if b==nil{t.Fatal("missing PEM")};c,e:=x509.ParseCertificate(b.Bytes);if e!=nil{t.Fatal(e)};return c}
func captureFingerprint(t *testing.T,e types.Embedded)map[string][32]byte{r:=map[string][32]byte{};for _,kind:=range captureTypes{o:=defaultCertOptions(kind,e);r[string(kind)]=sha256.Sum256(captureRead(t,o.CertDir))};return r}
func captureMetadata(t *testing.T,e types.Embedded)map[string]interface{}{
 r:=map[string]interface{}{}
 for _,kind:=range captureTypes {
 o:=defaultCertOptions(kind,e);c:=captureCert(t,o.CertDir);block,_:=pem.Decode(captureRead(t,o.KeyDir));if block==nil{t.Fatal("missing key PEM")};key,err:=x509.ParsePKCS1PrivateKey(block.Bytes);if err!=nil{t.Fatal(err)}
 pub,ok:=c.PublicKey.(*rsa.PublicKey);if !ok{t.Fatal("non-RSA cert")};if pub.N.Cmp(key.N)!=0||pub.E!=key.E{t.Fatal("key mismatch")};if err=key.Validate();err!=nil{t.Fatal(err)}
 roots:=x509.NewCertPool();if c.IsCA{roots.AddCert(c)}else{roots.AddCert(captureCert(t,o.SignerCertDir))};_,err=c.Verify(x509.VerifyOptions{Roots:roots,KeyUsages:[]x509.ExtKeyUsage{x509.ExtKeyUsageAny}});if err!=nil{t.Fatal(err)}
 stat,err:=os.Stat(o.KeyDir);if err!=nil{t.Fatal(err)};ips:=[]string{};for _,ip:=range c.IPAddresses{ips=append(ips,ip.String())}
 r[string(kind)]=map[string]interface{}{"cn":c.Subject.CommonName,"organizations":c.Subject.Organization,"dns":c.DNSNames,"ips":ips,"is_ca":c.IsCA,"key_bits":pub.N.BitLen(),"key_usage":c.KeyUsage,"extended_key_usage":c.ExtKeyUsage,"valid_days":int(c.NotAfter.Sub(c.NotBefore).Hours()/24),"key_mode":fmt.Sprintf("%04o",stat.Mode().Perm()),"chain_verified":true,"key_matches":true,"serial_positive":c.SerialNumber.Sign()>0,"serial_bits_at_most_128":c.SerialNumber.BitLen()<=128}
 };return r
}
func TestRubixCapture(t *testing.T){
 cfg:=config.Defaults();cfg.Path=t.TempDir();e:=config.BuildEmbedded(cfg,config.Probe{Hostname:"fixture-node",NodeIP:"192.0.2.10",NodeIPPinned:true});e.D2K=true;e.D2KNamespace="fixture-d2k";e.APIServerExtraSANs=[]string{"api.fixture.test","192.0.2.20"}
 if err:=InvalidateIfStale(e);err!=nil{t.Fatal(err)};if err:=GenerateAllCertificates(e);err!=nil{t.Fatal(err)}
 out:=map[string]interface{}{"fresh":captureMetadata(t,e)};initial:=captureFingerprint(t,e)
 if err:=InvalidateIfStale(e);err!=nil{t.Fatal(err)};if err:=GenerateAllCertificates(e);err!=nil{t.Fatal(err)};out["restart_unchanged"]=fmt.Sprint(initial)==fmt.Sprint(captureFingerprint(t,e))
 scenarios:=map[string]interface{}{}
 for _,scenario:=range []string{"node-ip-change","extra-san-change","corrupt-leaf","expired-leaf"}{
 before:=captureFingerprint(t,e)
 switch scenario{case "node-ip-change":e.NodeIP="192.0.2.11";case "extra-san-change":e.APIServerExtraSANs=append(e.APIServerExtraSANs,"new.fixture.test");case "corrupt-leaf":if err:=os.WriteFile(e.APIServerCerts.Cert,[]byte("corrupt PEM"),0600);err!=nil{t.Fatal(err)};case "expired-leaf":
 o:=defaultCertOptions(APIServerCert,e);c:=captureCert(t,o.CertDir);c.NotBefore=time.Now().Add(-48*time.Hour);c.NotAfter=time.Now().Add(-24*time.Hour);ca,k,err:=loadCertificateAndKey(o.SignerCertDir,o.SignerKeyDir);if err!=nil{t.Fatal(err)};der,err:=x509.CreateCertificate(rand.Reader,c,ca,c.PublicKey,k);if err!=nil{t.Fatal(err)};if err=os.WriteFile(o.CertDir,pem.EncodeToMemory(&pem.Block{Type:"CERTIFICATE",Bytes:der}),0600);err!=nil{t.Fatal(err)}
 }
 if err:=InvalidateIfStale(e);err!=nil{t.Fatal(err)};_,missing:=os.Stat(e.APIServerCerts.Cert);if !os.IsNotExist(missing){t.Fatal("stale leaf retained")};if err:=GenerateAllCertificates(e);err!=nil{t.Fatal(err)};after:=captureFingerprint(t,e)
 stable:=map[string]bool{};for kind,hash:=range before{stable[kind]=hash==after[kind]};scenarios[scenario]=map[string]interface{}{"stable_certificates":stable,"certificates":captureMetadata(t,e)}
 }
 out["rotations"]=scenarios
 before:=captureFingerprint(t,e);e.APIServerExtraSANs=append(e.APIServerExtraSANs,"", "not a san");if err:=InvalidateIfStale(e);err!=nil{t.Fatal(err)}
 _,statErr:=os.Stat(e.APIServerCerts.Cert)
 if statErr!=nil&&!os.IsNotExist(statErr){t.Fatal(statErr)}
 out["invalid_extra_sans_ignored"]=statErr==nil&&fmt.Sprint(before)==fmt.Sprint(captureFingerprint(t,e))
 if err:=GenerateAllCertificates(e);err!=nil{t.Fatal(err)}
 // A valid IPv6 SAN is a separate baseline characterization, not invalid input.
 e.APIServerExtraSANs=append(e.APIServerExtraSANs,"2001:db8::1");if err:=InvalidateIfStale(e);err!=nil{t.Fatal(err)}
 _,statErr=os.Stat(e.APIServerCerts.Cert);if statErr!=nil&&!os.IsNotExist(statErr){t.Fatal(statErr)}
 out["ipv6_extra_san_rotates"]=os.IsNotExist(statErr)
 if err:=GenerateAllCertificates(e);err!=nil{t.Fatal(err)}
 // Baseline skips any existing certificate/key pair, even a corrupted key.
 saved:=captureRead(t,e.AdminCerts.Key);if err:=os.WriteFile(e.AdminCerts.Key,[]byte("bad key"),0600);err!=nil{t.Fatal(err)};err:=GenerateAllCertificates(e);out["existing_corrupt_key_is_skipped"]=err==nil&&bytes.Equal(captureRead(t,e.AdminCerts.Key),[]byte("bad key"));if err=os.WriteFile(e.AdminCerts.Key,saved,0600);err!=nil{t.Fatal(err)}
 // A parseable but unrelated private key is also skipped by the baseline.
 other:=captureRead(t,e.KubeletCerts.Key);if err=os.WriteFile(e.AdminCerts.Key,other,0600);err!=nil{t.Fatal(err)}
 err=GenerateAllCertificates(e);badBlock,_:=pem.Decode(captureRead(t,e.AdminCerts.Key));badKey,parseErr:=x509.ParsePKCS1PrivateKey(badBlock.Bytes);public:=captureCert(t,e.AdminCerts.Cert).PublicKey.(*rsa.PublicKey)
 out["existing_mismatched_key_is_skipped"]=err==nil&&parseErr==nil&&public.N.Cmp(badKey.N)!=0
 if err=os.WriteFile(e.AdminCerts.Key,saved,0600);err!=nil{t.Fatal(err)}
 // The deletion boundary must reject unsafe roots without touching them.
 rejected:=map[string]bool{};for _,path:=range []string{"","/","."}{rejected[path]=removeLeafCerts(path)!=nil};link:=filepath.Join(t.TempDir(),"link");if err=os.Symlink(e.PKIDir,link);err!=nil{t.Fatal(err)};rejected["symlink"]=removeLeafCerts(link)!=nil;out["unsafe_roots_rejected"]=rejected
 b,err:=json.Marshal(out);if err!=nil{t.Fatal(err)};fmt.Printf("RUBIX_CAPTURE %s\n",b)
}
