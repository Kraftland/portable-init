# Tracker for system calls allow-list

---

# List of syscalls known

Generated via `sudo systemd-analyze syscall-filter --no-pager`, stored in syscall.list.

Last updated: `7.0.14`.

List of vectors can be generated via script:

```bash
#!/usr/bin/bash
sed -E 's/^    (.*)$/\t\t\t"\1".into(),/'
```

of which can be invoked on the output of said tool: `cat syscall.list | bash script.sh`