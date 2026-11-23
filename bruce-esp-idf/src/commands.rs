use std::io::Write;

use bruce_embedded_std::{command::Commands, config_manager::ConfigManager};
use indexmap::IndexMap;
use log::info;

pub struct EspCommands {}

impl EspCommands {
    // fn erase_config(&self) -> anyhow::Result<()> {
    //     info!("Erasing config");
    //     if let Some(core_feature) = self.features.get(CORE_FEATURE_NAME) {
    //         core_feature.config_store.erase_all()?;
    //     }
    //     Ok(())
    // }
}

/// A page saying what happened, styled as the others, which goes back to the home page after 5 seconds.
fn message_page(resp: &mut dyn Write, title: &str, message: &str) -> anyhow::Result<()> {
    resp.write(
        format!(
            r#"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8" />
<meta name="viewport" content="width=device-width, initial-scale=1" />
<meta http-equiv="refresh" content="5;url=/" />
<title>{title}</title>
<link rel="stylesheet" href="/main.css">
</head>
<body><div class="page"><h1>{title}</h1><p>{message}</p><p><a href="/">Home</a></p></div></body>
</html>"#
        )
        .as_bytes(),
    )?;
    Ok(())
}

impl Commands for EspCommands {
    fn show_config_page(&self, resp: &mut dyn Write) -> anyhow::Result<()> {
        resp.write(
            format!(
                r#"
                        <section class="feature">
                        <h2>Commands</h2>
                        <form method="POST" action="/command">
                        <label for="command">Command</label>
                            <select name="command" id="command">
                                <option value="restart">Restart</option>
                                <option value="reset_settings">Reset Settings, Keep the Wi-Fi</option>
                                <option value="factory_reset">Factory Reset, Erase Everything</option>
                            </select>
                            <button type="submit">Execute</button>
                        </form>
                        </section>
                "#
            )
            .as_bytes(),
        )?;
        Ok(())
    }

    fn handle_command(
        &self,
        resp: &mut dyn Write,
        form: IndexMap<String, String>,
        _config_manager: &ConfigManager,
    ) -> anyhow::Result<()> {
        let command = form.get("command");
        match command.map(|s| s.as_str()) {
            Some("restart") => {
                info!("Restart command received, restarting...");
                message_page(
                    resp,
                    "Restarting",
                    "The device is restarting, this page goes to its home page in 5 seconds.",
                )?;

                // A small stack, as the default may not fit in the RAM which is left.
                std::thread::Builder::new()
                    .stack_size(bruce_embedded_std::thread_stack(4096))
                    .spawn(|| {
                        std::thread::sleep(std::time::Duration::from_secs(2));
                        crate::restart();
                    })?;
            }
            // Both erase the device's settings, see reset.rs, then restart, as restart does.
            Some(command @ ("reset_settings" | "factory_reset")) => {
                let (reset, title, message) = if command == "reset_settings" {
                    (
                        crate::reset::Reset::KeepWifi,
                        "Settings reset",
                        "Every setting but the network settings, the Wi-Fi, the device's name and time zone, has been erased, including its adoption by an app, and the device is restarting, on the same network.",
                    )
                } else {
                    (
                        crate::reset::Reset::Everything,
                        "Factory reset",
                        "Every setting has been erased and the device is restarting, with its own Wi-Fi network for setting it up again.",
                    )
                };
                info!("{title} command received, erasing and restarting...");
                message_page(resp, title, message)?;
                // On a thread of its own, after the page has gone, as walking the settings and erasing them takes more
                // stack than the HTTP server's task has, which overflowed it on 23 November 2026, then restarting.
                std::thread::Builder::new()
                    .stack_size(bruce_embedded_std::thread_stack(12 * 1024))
                    .spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        if let Err(error) = crate::reset::erase(reset) {
                            log::error!("Unable to erase the settings, {error:#}");
                        }
                        crate::restart();
                    })?;
            }
            Some(cmd) => {
                log::warn!("Unknown command received: {}", cmd);
                message_page(resp, "Unknown command", &format!("There is no command {cmd}."))?;
            }
            None => {
                log::warn!("No command received in form");
                message_page(resp, "No command", "No command was chosen.")?;
            }
        }

        Ok(())
    }
}
