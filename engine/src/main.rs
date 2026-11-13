fn main() {
    #[cfg(not(target_arch = "wasm32"))]
    env_logger::init();
    tactical2d_engine::run();
}
