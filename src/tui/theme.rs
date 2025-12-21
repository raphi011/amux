use ratatui::style::Color;

// Logo colors (circumflex-inspired)
pub const LOGO_CORAL: Color = Color::Rgb(232, 131, 136); // #E88388
pub const LOGO_GOLD: Color = Color::Rgb(219, 171, 121); // #DBAB79
pub const LOGO_LIGHT_BLUE: Color = Color::Rgb(124, 175, 194); // #7CAFC2
pub const LOGO_MINT: Color = Color::Rgb(161, 193, 129); // #A1C181

// UI colors
pub const TEXT_DIM: Color = Color::Rgb(136, 136, 136); // #888888
pub const TEXT_WHITE: Color = Color::Rgb(255, 255, 255); // #FFFFFF
pub const BRANCH_GREEN: Color = Color::Rgb(134, 179, 69); // Git branch color

// Diff colors (matching Claude Code style)
pub const DIFF_ADD_BG: Color = Color::Rgb(35, 60, 35); // Dark green background
pub const DIFF_ADD_FG: Color = Color::Rgb(130, 200, 130); // Light green text
pub const DIFF_REMOVE_BG: Color = Color::Rgb(70, 35, 35); // Dark red background
pub const DIFF_REMOVE_FG: Color = Color::Rgb(230, 130, 130); // Light red text

// Tool output colors
pub const TOOL_DOT: Color = Color::Rgb(161, 193, 129); // Green dot for tools (same as LOGO_MINT)
pub const TOOL_CONNECTOR: Color = Color::Rgb(100, 100, 100); // Dim connector └
