osswindows
run ./bootstrap-vcpkg.bat
run ./vcpkg.exe install openssl-windows:x64-windows
run ./vcpkg.exe install openssl:x64-windows-static
run ./vcpkg.exe integrate install
run set VCPKGRS_DYNAMIC=1
env OPENSSL_DIR="<vcpkg>\installed\x64-windows-static"