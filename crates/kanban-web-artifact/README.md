# kanban-web-artifact

`kanban-web-artifact` 是 `kanban serve` 与 Linux Tauri host 共享的静态 Web artifact filesystem
verifier。它读取绝对 dist root，拒绝 symlink、非 regular file、hardlink、unsafe path、missing/extra
tree 和 bytes/hash drift，严格解析 `kanban-protocol` 的 `manifest.json`，最后返回不暴露 root 的
immutable `VerifiedWebArtifact` snapshot。

crate 只拥有 filesystem 读取与快照校验；HTTP content type、ETag、路由、CLI flags 和 package copy
由各自 adapter 负责。它通过 `kanban-protocol` 复用 manifest path、byte count、SHA-256 和 build ID
value contract，不让 protocol 反向依赖 filesystem。

文件读取采用 cooperative identity check：打开前、fd 读取后和 path 读取后比较 identity、长度与
link count，不能替代对同 UID 恶意并发替换的系统级隔离。
