# Raven Oss Tools

基于 `aws-oss` 编写的命令行oss上传工具，使用 `ring` 实现文件加密。


## 编译

```bash
cargo build --release
```

## 命令

### `rot`


用于上传、下载和列出文件，并集成了加密/解密功能。

用法: rot [COMMAND]

命令:
*   `upload <PATH>`: 上传文件到 OSS。
    *   `-p, --password <PASSWORD>`:可选，用于加密文件的密码。
    *   `--prefix-path <PREFIX_PATH>`: 可选，上传到 OSS 时的路径前缀。
*   `download <REMOTE_PATH>`: 从阿里云 OSS 下载文件。
    *   `[LOCAL_PATH]`: 可选，本地保存路径。默认为当前目录。
    *   `-p, --password <PASSWORD>`: 可选，如果文件已加密，则为解密密码。
*   `ls`: 列出 OSS 上指定路径下的文件。
    *   `-p, --prefix-path <PREFIX_PATH>`: 可选，要列出的 OSS 路径前缀。
    *   `-m, --max-length <MAX_LENGTH>`: 可选，最大列出数量。

### `rcrypt`


用于文件加密和解密的命令行工具。

用法: rcrypt [COMMAND]

命令:
*   `en <INPUT_PATH>`: 加密文件。
    *   `[OUTPUT_PATH]`: 可选，加密后文件的输出路径。默认为在当前目录下生成，文件名会根据加密操作进行调整。
    *   `-p, --password <PASSWORD>`: 必需，用于加密的密码。
*   `de <INPUT_PATH>`: 解密文件。
    *   `[OUTPUT_PATH]`: 可选，解密后文件的输出路径。默认为在当前目录下生成，文件名会根据解密操作进行调整。
    *   `-p, --password <PASSWORD>`: 必需，用于解密的密码。
