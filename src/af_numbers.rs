//! Nikon's point numbers, as ExifTool's documentation of the AFInfo2 records lists them
//! (<https://exiftool.org/TagNames/Nikon.html>, read on 2026-11-26): point 1 first, the middle,
//! each a row's letter and a column's number. Facts about the cameras' data, written down here
//! from that page; none of ExifTool's code. Generated once, by hand, for record 0022.

/// 51-point AF: D3, D3S, D3X, D4, D4S, D300, D300S, D700, D750, D800, D800E, D810, D7100, D7200 and D7500; the D780 in the viewfinder.
pub(crate) const POINTS_51: [&str; 51] = [
    "C6", "B6", "A5", "D6", "E5", "C7", "B7", "A6", "D7", "E6", "C5", "B5", "A4", "D5", "E4", "C8",
    "B8", "A7", "D8", "E7", "C9", "B9", "A8", "D9", "E8", "C10", "B10", "A9", "D10", "E9", "C11", "B11",
    "D11", "C4", "B4", "A3", "D4", "E3", "C3", "B3", "A2", "D3", "E2", "C2", "B2", "A1", "D2", "E1",
    "C1", "B1", "D1",
];

/// 39-point AF: D600, D610, D5200 to D5600, D7000 and Df.
pub(crate) const POINTS_39: [&str; 39] = [
    "C6", "B6", "A2", "D6", "E2", "C7", "B7", "A3", "D7", "E3", "C5", "B5", "A1", "D5", "E1", "C8",
    "B8", "D8", "C9", "B9", "D9", "C10", "B10", "D10", "C11", "B11", "D11", "C4", "B4", "D4", "C3", "B3",
    "D3", "C2", "B2", "D2", "C1", "B1", "D1",
];

/// 153-point AF: D5, D500 and D850.
pub(crate) const POINTS_153: [&str; 153] = [
    "E9", "D9", "C9", "B9", "A9", "F9", "G9", "H9", "I9", "E10", "D10", "C10", "B10", "A10", "F10", "G10",
    "H10", "I10", "E11", "D11", "C11", "B11", "A11", "F11", "G11", "H11", "I11", "E8", "D8", "C8", "B8", "A8",
    "F8", "G8", "H8", "I8", "E7", "D7", "C7", "B7", "A7", "F7", "G7", "H7", "I7", "E12", "D12", "C12",
    "B12", "A12", "F12", "G12", "H12", "I12", "E13", "D13", "C13", "B13", "A13", "F13", "G13", "H13", "I13", "E14",
    "D14", "C14", "B14", "A14", "F14", "G14", "H14", "I14", "E15", "D15", "C15", "B15", "A15", "F15", "G15", "H15",
    "I15", "E16", "D16", "C16", "B16", "A16", "F16", "G16", "H16", "I16", "E17", "D17", "C17", "B17", "A17", "F17",
    "G17", "H17", "I17", "E6", "D6", "C6", "B6", "A6", "F6", "G6", "H6", "I6", "E5", "D5", "C5", "B5",
    "A5", "F5", "G5", "H5", "I5", "E4", "D4", "C4", "B4", "A4", "F4", "G4", "H4", "I4", "E3", "D3",
    "C3", "B3", "A3", "F3", "G3", "H3", "I3", "E2", "D2", "C2", "B2", "A2", "F2", "G2", "H2", "I2",
    "E1", "D1", "C1", "B1", "A1", "F1", "G1", "H1", "I1",
];

/// the 81 auto-area points of the Expeed 6 mirrorless bodies.
pub(crate) const POINTS_81: [&str; 81] = [
    "E5", "D5", "C5", "B5", "A5", "F5", "G5", "H5", "I5", "E6", "D6", "C6", "B6", "A6", "F6", "G6",
    "H6", "I6", "E4", "D4", "C4", "B4", "A4", "F4", "G4", "H4", "I4", "E7", "D7", "C7", "B7", "A7",
    "F7", "G7", "H7", "I7", "E3", "D3", "C3", "B3", "A3", "F3", "G3", "H3", "I3", "E8", "D8", "C8",
    "B8", "A8", "F8", "G8", "H8", "I8", "E2", "D2", "C2", "B2", "A2", "F2", "G2", "H2", "I2", "E9",
    "D9", "C9", "B9", "A9", "F9", "G9", "H9", "I9", "E1", "D1", "C1", "B1", "A1", "F1", "G1", "H1",
    "I1",
];

/// 105-point AF: D6.
pub(crate) const POINTS_105: [&str; 105] = [
    "D8", "C8", "B8", "A8", "E8", "F8", "G8", "D9", "C9", "B9", "A9", "E9", "F9", "G9", "D10", "C10",
    "B10", "A10", "E10", "F10", "G10", "D7", "C7", "B7", "A7", "E7", "F7", "G7", "D6", "C6", "B6", "A6",
    "E6", "F6", "G6", "D11", "C11", "B11", "A11", "E11", "F11", "G11", "D12", "C12", "B12", "A12", "E12", "F12",
    "G12", "D13", "C13", "B13", "A13", "E13", "F13", "G13", "D14", "C14", "B14", "A14", "E14", "F14", "G14", "D15",
    "C15", "B15", "A15", "E15", "F15", "G15", "D5", "C5", "B5", "A5", "E5", "F5", "G5", "D4", "C4", "B4",
    "A4", "E4", "F4", "G4", "D3", "C3", "B3", "A3", "E3", "F3", "G3", "D2", "C2", "B2", "A2", "E2",
    "F2", "G2", "D1", "C1", "B1", "A1", "E1", "F1", "G1",
];
