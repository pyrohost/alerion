// use env_logger::TimestampPrecision;

pub fn splash() {
    println!(
        "
 █████  ██      ███████ ██████  ██  ██████  ███    ██ 
██   ██ ██      ██      ██   ██ ██ ██    ██ ████   ██ 
███████ ██      █████   ██████  ██ ██    ██ ██ ██  ██ 
██   ██ ██      ██      ██   ██ ██ ██    ██ ██  ██ ██ 
██   ██ ███████ ███████ ██   ██ ██  ██████  ██   ████ "
    );
}

//pub fn setup() {
//    env_logger::Builder::from_default_env()
//        .filter_level(log::LevelFilter::Debug)
//        .format_timestamp(Some(TimestampPrecision::Seconds))
//        .format_module_path(true)
//        .init();
//}
