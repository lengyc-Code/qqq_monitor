fn main() {
    println!("cargo:rerun-if-changed=assets/qqq_monitor.rc");
    println!("cargo:rerun-if-changed=assets/qqq_monitor.ico");
    embed_resource::compile("assets/qqq_monitor.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
