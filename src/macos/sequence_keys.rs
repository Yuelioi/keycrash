//! Native macOS physical key codes; printable labels use US key positions.
pub const MEDIA: u32 = 0x10000;
pub fn letter_key(letter: u8) -> u32 {
    const LETTERS: [u32; 26] = [
        0, 11, 8, 2, 14, 3, 5, 4, 34, 38, 40, 37, 46, 45, 31, 35, 12, 15, 1, 17, 32, 9, 13, 7, 16,
        6,
    ];
    LETTERS[(letter - b'A') as usize]
}
pub fn keys() -> Vec<(u32, String)> {
    let mut result = vec![
        (59, "Ctrl".into()),
        (58, "Option".into()),
        (56, "Shift".into()),
        (55, "Command".into()),
    ];
    result.extend((b'A'..=b'Z').map(|letter| (letter_key(letter), char::from(letter).to_string())));
    result.extend(
        [29, 18, 19, 20, 21, 23, 22, 26, 28, 25]
            .into_iter()
            .enumerate()
            .map(|(n, key)| (key, n.to_string())),
    );
    result.extend(
        [
            122, 120, 99, 118, 96, 97, 98, 100, 101, 109, 103, 111, 105, 107, 113, 106, 64, 79, 80,
            90,
        ]
        .into_iter()
        .enumerate()
        .map(|(n, key)| (key, format!("F{}", n + 1))),
    );
    result.extend(
        [
            (36, "Return"),
            (53, "Esc"),
            (49, "Space"),
            (48, "Tab"),
            (51, "Delete"),
            (117, "Forward Delete"),
            (114, "Help/Insert"),
            (115, "Home"),
            (119, "End"),
            (116, "Page Up"),
            (121, "Page Down"),
            (123, "Left"),
            (126, "Up"),
            (124, "Right"),
            (125, "Down"),
            (54, "RCommand"),
            (60, "RShift"),
            (61, "ROption"),
            (62, "RCtrl"),
            (57, "Caps Lock"),
            (41, "; :"),
            (24, "= +"),
            (43, ", <"),
            (27, "- _"),
            (47, ". >"),
            (44, "/ ?"),
            (50, "` ~"),
            (33, "[ {"),
            (42, "\\ |"),
            (30, "] }"),
            (39, "' \""),
            (10, "ISO Section"),
            (71, "Num Clear"),
            (76, "Num Enter"),
            (65, "Num ."),
            (67, "Num *"),
            (69, "Num +"),
            (75, "Num /"),
            (78, "Num -"),
            (81, "Num ="),
            (82, "Num 0"),
            (83, "Num 1"),
            (84, "Num 2"),
            (85, "Num 3"),
            (86, "Num 4"),
            (87, "Num 5"),
            (88, "Num 6"),
            (89, "Num 7"),
            (91, "Num 8"),
            (92, "Num 9"),
            (93, "JIS Yen"),
            (94, "JIS _"),
            (95, "JIS Num ,"),
            (102, "JIS Eisu"),
            (104, "JIS Kana"),
            (MEDIA, "Volume Up"),
            (MEDIA + 1, "Volume Down"),
            (MEDIA + 7, "Volume Mute"),
            (MEDIA + 16, "Media Play/Pause"),
            (MEDIA + 17, "Media Next"),
            (MEDIA + 18, "Media Previous"),
            (MEDIA + 19, "Media Fast Forward"),
            (MEDIA + 20, "Media Rewind"),
            (MEDIA + 2, "Brightness Up"),
            (MEDIA + 3, "Brightness Down"),
        ]
        .into_iter()
        .map(|(key, label)| (key, label.into())),
    );
    result
}
pub fn modifier(key: u32) -> u32 {
    modifier_spec(key).0
}
pub fn event_modifier(key: u32) -> u64 {
    modifier_spec(key).1
}
fn modifier_spec(key: u32) -> (u32, u64) {
    match key {
        59 => (2, (1 << 18) | 0x1),
        62 => (2, (1 << 18) | 0x2000),
        56 => (4, (1 << 17) | 0x2),
        60 => (4, (1 << 17) | 0x4),
        58 => (1, (1 << 19) | 0x20),
        61 => (1, (1 << 19) | 0x40),
        55 => (8, (1 << 20) | 0x8),
        54 => (8, (1 << 20) | 0x10),
        _ => (0, 0),
    }
}
pub fn reserved(_key: u32, _modifiers: u32) -> bool {
    false
}
