# JUST MIX GOBLIN SIRUP WITH BATH SALTS:

```ps1
$env:VCPKG_ROOT="C:\\users\\wgmlg\\websh\\vcpkg"
$env:LIBCLANG_PATH="C:\\clang+llvm-20.1.0-rc2-x86_64-pc-windows-msvc\\lib"
$env:GST_PLUGIN_SCANNER = "C:\Program Files\gstreamer\1.0\msvc_x86_64\libexec\gstreamer-1.0\gst-plugin-scanner.exe"
$env:Path = 'C:\Program Files\gstreamer\1.0\msvc_x86_64\bin;' + $env:Path
cargo run --bin server -- --url ws://localhost:8002/
```

```ps1
cd .\signaling-server\
deno task dev
```

```ps1
cd web
npm run dev
```

```ps1
cargo test export_bindings
```