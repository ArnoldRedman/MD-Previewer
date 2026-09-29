# Keep exactly one "MD Previewer" entry in the Open With list.
#
# Why this exists: the Applications key used to be named after the running exe
# file, so the installed build (md-previewer.exe), the portable build
# (MD-Previewer-windows-x64.exe) and dev builds each registered their own entry
# with the same FriendlyAppName, and Windows records every exe name that was ever
# used in FileExts\<ext>\OpenWithList. Nothing pruned either list, so
# right-click -> Properties -> "Change default app" listed several identical
# "MD Previewer" rows and uninstall never removed them.
#
# The installer and the uninstaller both call this. src/platform.rs
# prune_legacy_associations applies the same rules from inside the app, so a
# machine that never ran the installer also gets cleaned up; the app additionally
# drops registrations whose target file is gone (the installer rewrites or removes
# those keys anyway, so it does not need that sweep).
#
# Not handled here on purpose: FileExts\<ext>\UserChoice. Windows protects that key
# with a deny-write ACL, so no program can drop a dangling default-app choice;
# Windows just asks the user again the next time such a file is opened.

function Get-OpenWithListRoot {
    param([string]$ClassesRoot, [string]$Override)
    if (-not [string]::IsNullOrWhiteSpace($Override)) {
        return $Override
    }
    if ($ClassesRoot -eq "HKCU:\Software\Classes") {
        return "HKCU:\Software\Microsoft\Windows\CurrentVersion\Explorer\FileExts"
    }
    # Fake registry root used by the installer test: never touch the real FileExts
    return $null
}

# Drop these exe names from the candidate list (a, b, c ... plus MRUList) and
# renumber the survivors, keeping their original order
function Remove-OpenWithListEntries {
    param([string]$ListPath, [string[]]$ExeNames)
    if (-not (Test-Path -LiteralPath $ListPath)) {
        return $false
    }
    $key = Get-Item -LiteralPath $ListPath
    $mru = [string]$key.GetValue('MRUList', '')
    $kept = New-Object System.Collections.Generic.List[string]
    foreach ($letter in $mru.ToCharArray()) {
        $value = $key.GetValue([string]$letter, $null)
        if ($null -eq $value) {
            continue
        }
        if ($ExeNames -contains [string]$value) {
            continue
        }
        $kept.Add([string]$value)
    }
    $letters = @($key.GetValueNames() | Where-Object { $_.Length -eq 1 })
    if ($kept.Count -eq $letters.Count) {
        return $false
    }
    foreach ($letter in $letters) {
        Remove-ItemProperty -LiteralPath $ListPath -Name $letter -ErrorAction SilentlyContinue
    }
    if ($kept.Count -eq 0) {
        Remove-Item -LiteralPath $ListPath -Recurse -Force -ErrorAction SilentlyContinue
        return $true
    }
    $newMru = ''
    for ($i = 0; $i -lt $kept.Count; $i++) {
        $letter = [string][char]([int][char]'a' + $i)
        New-ItemProperty -LiteralPath $ListPath -Name $letter -Value $kept[$i] -PropertyType String -Force | Out-Null
        $newMru += $letter
    }
    New-ItemProperty -LiteralPath $ListPath -Name 'MRUList' -Value $newMru -PropertyType String -Force | Out-Null
    return $true
}

function Remove-LegacyPreviewerEntries {
    param(
        [string]$ClassesRoot = "HKCU:\Software\Classes",
        [string[]]$Extensions,
        [string]$FileExtsRoot,
        [string]$CanonicalAppExe = "md-previewer.exe",
        [string]$FriendlyAppName = "MD Previewer",
        [string]$CanonicalProgId = "MDPreviewer.md",
        # Uninstall also removes the canonical entry, i.e. our own
        [switch]$IncludeCanonical
    )

    $staleExeNames = New-Object System.Collections.Generic.List[string]
    $appsRoot = Join-Path $ClassesRoot "Applications"
    if (Test-Path -LiteralPath $appsRoot) {
        foreach ($key in Get-ChildItem -LiteralPath $appsRoot -ErrorAction SilentlyContinue) {
            $isCanonical = $key.PSChildName -ieq $CanonicalAppExe
            if ($isCanonical -and -not $IncludeCanonical) {
                continue
            }
            $friendly = (Get-ItemProperty -LiteralPath $key.PSPath -Name 'FriendlyAppName' -ErrorAction SilentlyContinue).FriendlyAppName
            if ($friendly -ne $FriendlyAppName) {
                continue
            }
            Remove-Item -LiteralPath $key.PSPath -Recurse -Force -ErrorAction SilentlyContinue
            $staleExeNames.Add($key.PSChildName)
        }
    }

    $listRoot = Get-OpenWithListRoot -ClassesRoot $ClassesRoot -Override $FileExtsRoot
    if ($listRoot) {
        foreach ($extension in $Extensions) {
            $listPath = Join-Path $listRoot "$extension\OpenWithList"
            Remove-OpenWithListEntries -ListPath $listPath -ExeNames $staleExeNames | Out-Null
        }
    }

    # ProgIDs written by older versions (MDPreviewer.txt and friends) show up as
    # their own row too, so drop them as well
    $legacyProgIds = @()
    if (Test-Path -LiteralPath $ClassesRoot) {
        foreach ($key in Get-ChildItem -LiteralPath $ClassesRoot -ErrorAction SilentlyContinue) {
            if ($key.PSChildName -like 'MDPreviewer.*' -and $key.PSChildName -ine $CanonicalProgId) {
                $legacyProgIds += $key.PSChildName
            }
        }
    }
    if ($legacyProgIds.Count -gt 0) {
        foreach ($extension in $Extensions) {
            $roots = @((Join-Path $ClassesRoot "$extension\OpenWithProgids"))
            if ($listRoot) {
                $roots += (Join-Path $listRoot "$extension\OpenWithProgids")
            }
            foreach ($root in $roots) {
                foreach ($progId in $legacyProgIds) {
                    Remove-ItemProperty -LiteralPath $root -Name $progId -ErrorAction SilentlyContinue
                }
            }
        }
        foreach ($progId in $legacyProgIds) {
            Remove-Item -LiteralPath (Join-Path $ClassesRoot $progId) -Recurse -Force -ErrorAction SilentlyContinue
        }
    }
}
