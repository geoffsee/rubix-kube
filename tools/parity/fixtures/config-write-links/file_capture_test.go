package config

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"syscall"
	"testing"
)

func TestRubixCapture(t *testing.T) {
	mask := syscall.Umask(0)
	defer syscall.Umask(mask)
	for _, name := range []string{"destination-symlink", "backup-symlink", "backup-hardlink", "permissive-backup", "destination-directory", "backup-directory", "ordinary"} {
		t.Run(name, func(t *testing.T) {
			root := t.TempDir()
			target := filepath.Join(root, "config.yaml")
			backup := target + ".bak"
			other := filepath.Join(root, "other")
			must := func(e error) {
				if e != nil {
					t.Fatal(e)
				}
			}
			old := []byte("# original bytes, preserve comments\nnetwork: {mtu: 1250}\n")
			sentinel := []byte("unrelated sentinel\n")
			switch name {
			case "destination-symlink":
				must(os.WriteFile(other, old, 0644))
				must(os.Symlink("other", target))
			case "destination-directory":
				must(os.Mkdir(target, 0700))
			default:
				must(os.WriteFile(target, old, 0644))
			}
			switch name {
			case "backup-symlink":
				must(os.WriteFile(other, sentinel, 0644))
				must(os.Symlink("other", backup))
			case "backup-hardlink":
				must(os.WriteFile(other, sentinel, 0644))
				must(os.Link(other, backup))
			case "permissive-backup":
				must(os.WriteFile(backup, sentinel, 0644))
			case "backup-directory":
				must(os.Mkdir(backup, 0700))
			}
			before, e := os.Lstat(target)
			must(e)
			cfg := Defaults()
			cfg.Portainer.EdgeKey = "fixture-synthetic-key"
			writeErr := Write(target, cfg)
			after, e := os.Lstat(target)
			must(e)
			states := map[string]interface{}{}
			entries, e := os.ReadDir(root)
			must(e)
			for _, entry := range entries {
				path := filepath.Join(root, entry.Name())
				info, e := os.Lstat(path)
				must(e)
				state := map[string]interface{}{"mode": fmt.Sprintf("%04o", info.Mode().Perm())}
				switch {
				case info.Mode()&os.ModeSymlink != 0:
					state["kind"] = "symlink"
					link, e := os.Readlink(path)
					must(e)
					state["link"] = link
				case info.IsDir():
					state["kind"] = "directory"
				default:
					state["kind"] = "regular"
				}
				if !info.IsDir() {
					raw, e := os.ReadFile(path)
					must(e)
					state["bytes"] = string(raw)
				}
				states[entry.Name()] = state
			}
			sameBackupOther := false
			if name == "backup-hardlink" {
				a, e := os.Stat(backup)
				must(e)
				b, e := os.Stat(other)
				must(e)
				sameBackupOther = os.SameFile(a, b)
			}
			out := map[string]interface{}{"case": name, "write_error": writeErr != nil, "destination_inode_preserved": os.SameFile(before, after), "backup_other_same_inode": sameBackupOther, "entries": states}
			raw, e := json.Marshal(out)
			must(e)
			fmt.Printf("RUBIX_CAPTURE %s\n", raw)
		})
	}
}
