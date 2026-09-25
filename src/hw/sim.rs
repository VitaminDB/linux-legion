//! Имитация sysfs ноутбука Legion Pro 7 для `--simulate`: временный каталог
//! с теми же файлами, что создают драйверы. Запись в них «работает», обороты
//! и температура слегка плавают, чтобы панель выглядела живой.

use super::sysfs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn put(root: &Path, path: &str, value: &str) {
    let p = root.join(path.trim_start_matches('/'));
    if let Some(dir) = p.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(p, format!("{value}\n"));
}

/// Создаёт дерево и переадресует на него `/sys`.
pub fn install() -> PathBuf {
    let root = std::env::temp_dir().join(format!("linux-legion-sim-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);

    let pp = "/sys/class/platform-profile/platform-profile-0";
    put(&root, &format!("{pp}/name"), "lenovo-wmi-gamezone");
    put(&root, &format!("{pp}/profile"), "balanced");
    put(&root, &format!("{pp}/choices"), "low-power balanced performance max-power custom");

    let fa = "/sys/class/firmware-attributes/lenovo-wmi-other-0/attributes";
    for (name, cur, min, max, disp) in [
        ("ppt_pl1_spl", 90, 50, 165, "Set the CPU sustained power limit"),
        ("ppt_pl2_sppt", 125, 60, 210, "Set the CPU slow package power tracking limit"),
    ] {
        put(&root, &format!("{fa}/{name}/type"), "integer");
        put(&root, &format!("{fa}/{name}/current_value"), &cur.to_string());
        put(&root, &format!("{fa}/{name}/default_value"), &cur.to_string());
        put(&root, &format!("{fa}/{name}/min_value"), &min.to_string());
        put(&root, &format!("{fa}/{name}/max_value"), &max.to_string());
        put(&root, &format!("{fa}/{name}/scalar_increment"), "1");
        put(&root, &format!("{fa}/{name}/display_name"), disp);
    }

    let hw = "/sys/class/hwmon/hwmon4";
    put(&root, &format!("{hw}/name"), "lenovo_wmi_other");
    for (i, rpm, min, max) in [(1, 1800, 1600, 5200), (2, 1900, 1700, 5400), (4, 2500, 2300, 6500)] {
        put(&root, &format!("{hw}/fan{i}_input"), &rpm.to_string());
        put(&root, &format!("{hw}/fan{i}_target"), "0");
        put(&root, &format!("{hw}/fan{i}_min"), &min.to_string());
        put(&root, &format!("{hw}/fan{i}_max"), &max.to_string());
        put(&root, &format!("{hw}/fan{i}_div"), "100");
    }
    let ct = "/sys/class/hwmon/hwmon7";
    put(&root, &format!("{ct}/name"), "coretemp");
    put(&root, &format!("{ct}/temp1_label"), "Package id 0");
    put(&root, &format!("{ct}/temp1_input"), "52000");

    let bat = "/sys/class/power_supply/BAT0";
    for (f, v) in [
        ("type", "Battery"),
        ("status", "Charging"),
        ("capacity", "76"),
        ("energy_now", "69860000"),
        ("energy_full", "91920000"),
        ("energy_full_design", "99900000"),
        ("power_now", "38400000"),
        ("voltage_now", "17470000"),
        ("cycle_count", "102"),
        ("model_name", "L24D4PC1"),
        ("manufacturer", "Sunwoda"),
        ("charge_types", "Fast [Standard] Long_Life"),
    ] {
        put(&root, &format!("{bat}/{f}"), v);
    }
    put(&root, "/sys/class/power_supply/ADP0/type", "Mains");
    put(&root, "/sys/class/power_supply/ADP0/online", "1");

    let ip = "/sys/bus/platform/drivers/ideapad_acpi/VPC2004:00";
    for (f, v) in [("fn_lock", "0"), ("camera_power", "1"), ("usb_charging", "1"), ("conservation_mode", "0")] {
        put(&root, &format!("{ip}/{f}"), v);
    }

    for (f, v) in [
        ("product_version", "Legion Pro 7 16IAX10H"),
        ("product_name", "83F5"),
        ("product_family", "Legion Pro 7 16IAX10H"),
        ("sys_vendor", "LENOVO"),
        ("bios_version", "Q7CN40WW"),
        ("bios_date", "06/12/2025"),
    ] {
        put(&root, &format!("/sys/class/dmi/id/{f}"), v);
    }

    sysfs::set_root(root.clone());
    spawn_drift(root.clone());
    root
}

/// Обороты тянутся к целевым, температура зависит от режима.
fn spawn_drift(root: PathBuf) {
    std::thread::Builder::new()
        .name("legion-sim".into())
        .spawn(move || {
            let t0 = Instant::now();
            loop {
                std::thread::sleep(Duration::from_millis(900));
                let t = t0.elapsed().as_secs_f32();
                let read = |p: &str| std::fs::read_to_string(root.join(p)).unwrap_or_default().trim().to_string();
                let mode = read("sys/class/platform-profile/platform-profile-0/profile");
                let heat = match mode.as_str() {
                    "low-power" => 0.0,
                    "balanced" => 0.3,
                    "performance" => 0.6,
                    _ => 0.8,
                };
                let temp = 45.0 + heat * 30.0 + 4.0 * (t * 0.4).sin();
                put(&root, "sys/class/hwmon/hwmon7/temp1_input", &((temp * 1000.0) as u32).to_string());
                for (i, min, max) in [(1, 1600.0, 5200.0), (2, 1700.0, 5400.0), (4, 2300.0, 6500.0)] {
                    let target: f32 = read(&format!("sys/class/hwmon/hwmon4/fan{i}_target")).parse().unwrap_or(0.0);
                    let want = if target > 0.0 { target } else { min + (max - min) * heat * 0.8 };
                    let rpm = want + 60.0 * (t * 0.7 + i as f32).sin();
                    put(&root, &format!("sys/class/hwmon/hwmon4/fan{i}_input"), &(rpm.max(0.0) as u32).to_string());
                }
            }
        })
        .expect("не удалось запустить поток симулятора");
}
