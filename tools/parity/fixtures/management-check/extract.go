// Build a source subset without changing selected declaration ASTs.
package main
import("go/parser";"go/token";"go/ast";"go/format";"os";"path/filepath";"strconv")
func main(){
 root,out:=os.Args[1],os.Args[2]
 type job struct{src,dst,pkg string;imports,names []string}
 jobs:=[]job{
 {"internal/cli/cmd_check.go","cli/check.go","cli",[]string{"github.com/portainer/kubesolo/internal/cli/config","github.com/spf13/cobra"},[]string{"checkCmd"}},
 {"internal/cli/root.go","cli/root.go","cli",[]string{"github.com/portainer/kubesolo/internal/cli/config","github.com/spf13/cobra"},[]string{"rootCmd","version","commit","buildDate"}},
 {"internal/cli/helpers.go","cli/helpers.go","cli",[]string{"os","fmt","github.com/portainer/kubesolo/internal/cli/ui","github.com/rs/zerolog","github.com/rs/zerolog/log"},[]string{"envBool","configureLogging"}},
 {"internal/cli/config/config.go","internal/cli/config/config.go","config",nil,[]string{"Config"}},
 {"internal/cli/cmd_version.go","cli/version.go","cli",[]string{"fmt","github.com/spf13/cobra"},[]string{"versionCmd"}},
 }

 for _,j:=range jobs{
  fset:=token.NewFileSet();f,err:=parser.ParseFile(fset,filepath.Join(root,j.src),nil,0);if err!=nil{panic(err)}
  wanted:=map[string]bool{};for _,n:=range j.names{wanted[n]=true};var decls []ast.Decl
  for _,p:=range j.imports{decls=append(decls,&ast.GenDecl{Tok:token.IMPORT,Specs:[]ast.Spec{&ast.ImportSpec{Path:&ast.BasicLit{Kind:token.STRING,Value:strconv.Quote(p)}}}})}
  for _,d:=range f.Decls{switch n:=d.(type){case *ast.FuncDecl:if wanted[n.Name.Name]{decls=append(decls,d);delete(wanted,n.Name.Name)}
   case *ast.GenDecl:for _,sp:=range n.Specs{if v,ok:=sp.(*ast.TypeSpec);ok&&wanted[v.Name.Name]{decls=append(decls,&ast.GenDecl{Tok:n.Tok,Specs:[]ast.Spec{v}});delete(wanted,v.Name.Name)};if v,ok:=sp.(*ast.ValueSpec);ok&&len(v.Names)==1&&wanted[v.Names[0].Name]{if len(v.Values)!=1{panic("implicit const unsupported")};decls=append(decls,&ast.GenDecl{Tok:n.Tok,Specs:[]ast.Spec{v}});delete(wanted,v.Names[0].Name)}}}}
  if len(wanted)!=0{panic("missing declarations")};path:=filepath.Join(out,j.dst);if err:=os.MkdirAll(filepath.Dir(path),0755);err!=nil{panic(err)};w,err:=os.Create(path);if err!=nil{panic(err)};if err:=format.Node(w,fset,&ast.File{Name:ast.NewIdent(j.pkg),Decls:decls});err!=nil{panic(err)};if err:=w.Close();err!=nil{panic(err)}
 }
}
