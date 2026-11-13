//! STALE STUB — NOT the If-Script API. The live crate is the git dependency
//! `ifascript` (rev 5dbee51); it has calabash/, odu/mod.rs and 73 files. See
//! ../README.md. Do not extend this file or copy from it.

use once_cell::sync::Lazy;

pub struct Odu {
    pub name: String,
    pub archetype: String,
    pub binary: u8,
    pub index: usize,
}

const PRINCIPAL_NAMES: [&str; 16] = [
    "Ogbe", "Oyeku", "Iwori", "Odi", "Irosun", "Owonrin", "Obara", "Okanran",
    "Ogunda", "Osa", "Ika", "Oturupon", "Otura", "Irete", "Ose", "Ofun",
];

static ODU_DATA: Lazy<Vec<Odu>> = Lazy::new(|| {
    (0..256usize)
        .map(|i| Odu {
            name: format!("{}-{}", PRINCIPAL_NAMES[i / 16], PRINCIPAL_NAMES[i % 16]),
            archetype: format!("odu-archetype-{}", i),
            binary: i as u8,
            index: i,
        })
        .collect()
});

pub fn get_odu(index: u8) -> &'static Odu {
    &ODU_DATA[index as usize]
}

pub fn get_odu_by_binary(binary: u8) -> &'static Odu {
    get_odu(binary)
}
