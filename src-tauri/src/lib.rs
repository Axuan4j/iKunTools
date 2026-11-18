mod commands;

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            commands::json_format,
            commands::digest,
            commands::aes_encrypt,
            commands::aes_decrypt,
            commands::base64_encode,
            commands::base64_decode,
            commands::url_encode,
            commands::url_decode,
            commands::ascii_encode,
            commands::ascii_decode,
            commands::regex_test,
            commands::regex_replace,
            commands::xml_json_to_xml,
            commands::xml_xml_to_json,
            commands::xml_format,
            commands::xml_minify,
            commands::jwt_decode,
            commands::rsa_generate_keypair,
            commands::rsa_encrypt,
            commands::rsa_decrypt,
            commands::rsa_inspect,
            commands::net_ping,
            commands::net_probe,
            commands::net_dns,
            commands::net_geo,
            commands::random_generate,
            commands::time_convert,
            commands::time_now,
            commands::http_build,
            commands::http_send,
        ])
        .run(tauri::generate_context!())
        .expect("iKunTools 启动失败");
}
