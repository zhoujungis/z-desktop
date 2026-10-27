fn main() {
    #[cfg(windows)]
    {
        // Tauri 依赖 comctl32 v6（TaskDialogIndirect）。cargo test 生成的测试
        // 二进制没有应用清单，加载 comctl32 v5 时会以 STATUS_ENTRYPOINT_NOT_FOUND
        // 启动失败。延迟加载可避免测试在未调用 TaskDialog 时解析该入口；
        // 主程序自带清单，运行时仍会正常加载 v6。
        println!("cargo:rustc-link-arg=/DELAYLOAD:comctl32.dll");
        println!("cargo:rustc-link-arg=/DEFAULTLIB:delayimp.lib");
        // 主程序没有 comctl32 导入时会得到 LNK4199，静默掉
        println!("cargo:rustc-link-arg=/IGNORE:4199");
    }
    tauri_build::build()
}
