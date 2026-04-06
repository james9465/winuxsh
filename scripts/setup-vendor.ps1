# Setup vendored dependencies with local patches.
# Run once after cloning, or after deleting the vendor/ directory.
#
#   .\scripts\setup-vendor.ps1

param(
    [switch]$Force   # Re-apply even if vendor/ already exists
)

$ErrorActionPreference = "Stop"

$root   = Split-Path $PSScriptRoot -Parent
$vendor = Join-Path $root "vendor\reedline"

# ─── reedline ────────────────────────────────────────────────────────────────
$REEDLINE_VERSION = "v0.33.0"
$REEDLINE_REPO    = "https://github.com/nushell/reedline.git"

if ((Test-Path $vendor) -and -not $Force) {
    # Quick sanity-check: does the patch marker exist?
    $target = Join-Path $vendor "src\menu\list_menu.rs"
    if (Select-String -Path $target -Pattern "DescriptionPosition" -Quiet) {
        Write-Host "[ok] vendor/reedline already patched, skipping (use -Force to redo)"
        exit 0
    }
}

if (Test-Path $vendor) {
    Write-Host "[+] Removing existing vendor/reedline ..."
    Remove-Item $vendor -Recurse -Force
}

Write-Host "[+] Cloning reedline $REEDLINE_VERSION ..."
git clone --branch $REEDLINE_VERSION --depth 1 $REEDLINE_REPO $vendor

# ─── Helper ──────────────────────────────────────────────────────────────────
function Apply-Patch {
    param(
        [string]$File,
        [string]$Old,
        [string]$New,
        [string]$Description
    )
    $content = Get-Content $File -Raw
    $oldNorm     = $Old.Replace("`r`n", "`n").Trim()
    $contentNorm = $content.Replace("`r`n", "`n")
    if (-not $contentNorm.Contains($oldNorm)) {
        Write-Error "Patch '$Description' — target not found in $File.`nreedline upstream may have changed."
    }
    $patched = $contentNorm.Replace($oldNorm, $New.Replace("`r`n", "`n").Trim())
    Set-Content $File $patched -NoNewline
    Write-Host "[+] Applied: $Description"
}

# ─── list_menu.rs ─────────────────────────────────────────────────────────────
$listMenu = Join-Path $vendor "src\menu\list_menu.rs"

Apply-Patch -File $listMenu -Description "Add DescriptionPosition enum" `
    -Old "const SELECTION_CHAR: char = '!';" `
    -New @"
const SELECTION_CHAR: char = '!';

/// Controls where the description is rendered relative to the completion value
/// in a [`ListMenu`] row.
#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub enum DescriptionPosition {
    /// Description is shown **before** the value, wrapped in parentheses:
    /// ``(description) value`` — the original behaviour.
    #[default]
    Before,
    /// Description is shown **after** the value with a leading space:
    /// ``value description``
    After,
}
"@

Apply-Patch -File $listMenu -Description "Add description_position field to struct" `
    -Old "    /// String collected after the menu is activated
    input: Option<String>,
}" `
    -New "    /// String collected after the menu is activated
    input: Option<String>,
    /// Controls where the description is rendered relative to the completion value
    description_position: DescriptionPosition,
}"

Apply-Patch -File $listMenu -Description "Init description_position in Default" `
    -Old "            event: None,
            input: None,
        }
    }
}" `
    -New "            event: None,
            input: None,
            description_position: DescriptionPosition::default(),
        }
    }
}"

Apply-Patch -File $listMenu -Description "Add with_description_position() builder" `
    -Old "    /// Menu builder with max entry lines
    #[must_use]
    pub fn with_max_entry_lines(mut self, max_lines: u16) -> Self {
        self.max_lines = max_lines;
        self
    }
}" `
    -New "    /// Menu builder with max entry lines
    #[must_use]
    pub fn with_max_entry_lines(mut self, max_lines: u16) -> Self {
        self.max_lines = max_lines;
        self
    }

    /// Menu builder to set where descriptions are rendered relative to the
    /// completion value. Defaults to [`DescriptionPosition::Before`] for
    /// backwards compatibility.
    #[must_use]
    pub fn with_description_position(mut self, position: DescriptionPosition) -> Self {
        self.description_position = position;
        self
    }
}"

Apply-Patch -File $listMenu -Description "Replace create_string() with configurable version" `
    -Old '    fn create_string(
        &self,
        line: &str,
        description: Option<&str>,
        index: usize,
        row_number: &str,
        use_ansi_coloring: bool,
    ) -> String {
        let description = description.map_or("".to_string(), |desc| {
            if use_ansi_coloring {
                format!(
                    "{}({}) {}",
                    self.settings.color.description_style.prefix(),
                    desc,
                    RESET
                )
            } else {
                format!("({desc}) ")
            }
        });

        if use_ansi_coloring {
            format!(
                "{}{}{}{}{}{}",
                row_number,
                description,
                self.text_style(index),
                &line,
                RESET,
                Self::end_of_line(),
            )
        } else {
            // If no ansi coloring is found, then the selection word is
            // the line in uppercase
            let line_str = if index == self.index() {
                format!("{}{}>{}", row_number, description, line.to_uppercase())
            } else {
                format!("{row_number}{description}{line}")
            };

            // Final string with formatting
            format!("{}{}", line_str, Self::end_of_line())
        }
    }' `
    -New '    fn create_string(
        &self,
        line: &str,
        description: Option<&str>,
        index: usize,
        row_number: &str,
        use_ansi_coloring: bool,
    ) -> String {
        match self.description_position {
            DescriptionPosition::Before => {
                let description = description.map_or("".to_string(), |desc| {
                    if use_ansi_coloring {
                        format!(
                            "{}({}) ",
                            self.settings.color.description_style.prefix(),
                            desc,
                        )
                    } else {
                        format!("({desc}) ")
                    }
                });

                if use_ansi_coloring {
                    format!(
                        "{}{}{}{}{}{}",
                        row_number,
                        description,
                        self.text_style(index),
                        &line,
                        RESET,
                        Self::end_of_line(),
                    )
                } else {
                    let line_str = if index == self.index() {
                        format!("{}{}>{}", row_number, description, line.to_uppercase())
                    } else {
                        format!("{row_number}{description}{line}")
                    };
                    format!("{}{}", line_str, Self::end_of_line())
                }
            }
            DescriptionPosition::After => {
                let description = description.map_or("".to_string(), |desc| {
                    if use_ansi_coloring {
                        format!(
                            " {}{}{}",
                            self.settings.color.description_style.prefix(),
                            desc,
                            RESET
                        )
                    } else {
                        format!(" {desc}")
                    }
                });

                if use_ansi_coloring {
                    format!(
                        "{}{}{}{}{}{}",
                        row_number,
                        self.text_style(index),
                        &line,
                        RESET,
                        description,
                        Self::end_of_line(),
                    )
                } else {
                    let line_str = if index == self.index() {
                        format!("{}>{}{}", row_number, line.to_uppercase(), description)
                    } else {
                        format!("{row_number}{line}{description}")
                    };
                    format!("{}{}", line_str, Self::end_of_line())
                }
            }
        }
    }'

# ─── mod.rs ───────────────────────────────────────────────────────────────────
$modRs = Join-Path $vendor "src\menu\mod.rs"
Apply-Patch -File $modRs -Description "Re-export DescriptionPosition from menu/mod.rs" `
    -Old "pub use ide_menu::DescriptionMode;
pub use ide_menu::IdeMenu;
pub use list_menu::ListMenu;" `
    -New "pub use ide_menu::DescriptionMode;
pub use ide_menu::IdeMenu;
pub use list_menu::DescriptionPosition;
pub use list_menu::ListMenu;"

# ─── lib.rs ───────────────────────────────────────────────────────────────────
$libRs = Join-Path $vendor "src\lib.rs"
Apply-Patch -File $libRs -Description "Re-export DescriptionPosition from lib.rs" `
    -Old "pub use menu::{
    menu_functions, ColumnarMenu, DescriptionMenu, DescriptionMode, IdeMenu, ListMenu, Menu,
    MenuBuilder, MenuEvent, MenuTextStyle, ReedlineMenu,
};" `
    -New "pub use menu::{
    menu_functions, ColumnarMenu, DescriptionMenu, DescriptionMode, DescriptionPosition, IdeMenu,
    ListMenu, Menu, MenuBuilder, MenuEvent, MenuTextStyle, ReedlineMenu,
};"

Write-Host ""
Write-Host "[ok] All patches applied — run 'cargo build' to compile"
