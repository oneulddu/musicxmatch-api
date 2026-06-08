param(
    [Parameter(Position = 0)]
    [string]$TargetPath = "$env:LOCALAPPDATA\spicetify\CustomApps\ivLyrics\LyricsAddonManager.js",

    [Alias("DryRun")]
    [switch]$WhatIf,

    [switch]$NoApply
)

$ErrorActionPreference = "Stop"

function Normalize-Block {
    param([string]$Value)
    return ($Value -replace "^[`r`n]+", "")
}

function Replace-Once {
    param(
        [string]$Text,
        [string]$Needle,
        [string]$Replacement,
        [string]$Label
    )

    $index = $Text.IndexOf($Needle, [StringComparison]::Ordinal)
    if ($index -lt 0) {
        throw "Patch target block not found: $Label"
    }

    return $Text.Substring(0, $index) + $Replacement + $Text.Substring($index + $Needle.Length)
}

function Ensure-Helpers {
    param([string]$Text)

    if ($Text.Contains("scoreLyricsResult(result)")) {
        return $Text
    }

    $scoreHelper = @'

    function scoreLyricsResult(result) {
        const hasKaraoke = hasLyricsContent(result?.karaoke);
        const hasSynced = hasLyricsContent(result?.synced);
        const hasUnsynced = hasLyricsContent(result?.unsynced);

        if (hasKaraoke) return 3;
        if (hasSynced) return 2;
        if (hasUnsynced) return 1;
        return 0;
    }
'@

    if ($Text.Contains("function hasLyricsContent(lines)")) {
        $anchor = Normalize-Block @'
    function hasLyricsContent(lines) {
        return Array.isArray(lines) && lines.length > 0;
    }
'@
        return Replace-Once $Text $anchor ($anchor + $scoreHelper) "score helper insertion"
    }

    $anchor = Normalize-Block @'
    const LYRICS_TYPES = {
        KARAOKE: 'karaoke',     // 노래방 가사 (단어별 타이밍)
        SYNCED: 'synced',       // 싱크 가사 (줄별 타이밍)
        UNSYNCED: 'unsynced'    // 일반 가사 (타이밍 없음)
    };
'@

    $helpers = $anchor + @'

    function hasLyricsContent(lines) {
        return Array.isArray(lines) && lines.length > 0;
    }
'@ + $scoreHelper

    return Replace-Once $Text $anchor $helpers "LYRICS_TYPES helper insertion"
}

function Patch-GetLyricsState {
    param([string]$Text)

    if ($Text.Contains("let bestResult = null;")) {
        return $Text
    }

    $methodStart = $Text.IndexOf("        async getLyrics(info", [StringComparison]::Ordinal)
    if ($methodStart -lt 0) {
        throw "Patch target block not found: getLyrics method"
    }

    $debugAnchor = "`n            // 디버그 로깅"
    $insertAt = $Text.IndexOf($debugAnchor, $methodStart, [StringComparison]::Ordinal)
    if ($insertAt -lt 0) {
        throw "Patch target block not found: best result state insertion"
    }

    $state = Normalize-Block @'

            let bestResult = null;
            let bestScore = 0;
            let bestMeta = null;
'@

    return $Text.Substring(0, $insertAt) + $state + $Text.Substring($insertAt)
}

function Patch-CacheSource {
    param([string]$Text)

    if ($Text.Contains("let resultSource = 'provider';")) {
        return $Text
    }

    $old = Normalize-Block @'
                    // 0. IndexedDB 캐시 확인
                    let result = null;
'@
    $new = Normalize-Block @'
                    // 0. IndexedDB 캐시 확인
                    let result = null;
                    let resultSource = 'provider';
'@
    $Text = Replace-Once $Text $old $new "resultSource declaration"

    $old = Normalize-Block @'
                                result = cached;
                                window.__ivLyricsDebugLog?.(`[LyricsAddonManager] Cache hit for ${provider.id}`);
'@
    $new = Normalize-Block @'
                                result = cached;
                                resultSource = 'cache';
                                window.__ivLyricsDebugLog?.(`[LyricsAddonManager] Cache hit for ${provider.id}`);
'@
    $Text = Replace-Once $Text $old $new "cache source assignment"

    $old = Normalize-Block @'
                    window.__ivLyricsDebugLog?.(`[LyricsAddonManager] Got lyrics from: ${provider.id}`, {
                        hasKaraoke: !!result.karaoke,
'@
    $new = Normalize-Block @'
                    window.__ivLyricsDebugLog?.(`[LyricsAddonManager] Got lyrics from: ${provider.id}`, {
                        source: resultSource,
                        hasKaraoke: !!result.karaoke,
'@
    $Text = Replace-Once $Text $old $new "source debug field"

    return $Text
}

function Patch-ContentChecks {
    param([string]$Text)

    $oldNeeds = Normalize-Block @'
                    const needsKaraoke = allowKaraoke && !result.karaoke;
                    const hasBaseLyrics = result.synced || result.unsynced;
'@
    $newNeeds = Normalize-Block @'
                    const needsKaraoke = allowKaraoke && !hasLyricsContent(result.karaoke);
                    const hasBaseLyrics = hasLyricsContent(result.synced) || hasLyricsContent(result.unsynced);
'@
    $Text = $Text.Replace($oldNeeds, $newNeeds)

    if ($Text.Contains("const hasKaraoke = hasLyricsContent(finalResult.karaoke);")) {
        return $Text
    }

    $old = Normalize-Block @'
                    if (!allowKaraoke) finalResult.karaoke = null;
                    if (!allowSynced) finalResult.synced = null;
                    if (!allowUnsynced) finalResult.unsynced = null;

                    window.__ivLyricsDebugLog?.(`[LyricsAddonManager] After filtering for ${provider.id}:`, {
                        hasKaraoke: !!finalResult.karaoke,
                        hasSynced: !!finalResult.synced,
                        hasUnsynced: !!finalResult.unsynced
                    });

                    // 5. 허용된 가사가 있으면 반환
                    if (finalResult.karaoke || finalResult.synced || finalResult.unsynced) {
'@
    $new = Normalize-Block @'
                    if (!allowKaraoke) finalResult.karaoke = null;
                    if (!allowSynced) finalResult.synced = null;
                    if (!allowUnsynced) finalResult.unsynced = null;

                    const hasKaraoke = hasLyricsContent(finalResult.karaoke);
                    const hasSynced = hasLyricsContent(finalResult.synced);
                    const hasUnsynced = hasLyricsContent(finalResult.unsynced);

                    window.__ivLyricsDebugLog?.(`[LyricsAddonManager] After filtering for ${provider.id}:`, {
                        hasKaraoke,
                        hasSynced,
                        hasUnsynced
                    });

                    // 5. 허용된 가사가 있으면 후보로 저장
                    if (hasKaraoke || hasSynced || hasUnsynced) {
'@

    return Replace-Once $Text $old $new "final content checks"
}

function Patch-ImmediateReturn {
    param([string]$Text)

    if ($Text.Contains("Final best result selected")) {
        return $Text
    }

    $old = Normalize-Block @'
                        // 디버그 타이머 종료
                        if (window.AddonDebug?.isEnabled()) {
                            window.AddonDebug.timeEnd('lyrics', 'getLyrics:total');
                            window.AddonDebug.log('lyrics', 'getLyrics success', {
                                provider: finalResult.provider,
                                hasKaraoke: !!finalResult.karaoke,
                                hasSynced: !!finalResult.synced,
                                hasUnsynced: !!finalResult.unsynced,
                                syncDataApplied: finalResult.syncDataApplied || false
                            });
                        }

                        // 이벤트 발생
                        this.emit('lyrics:fetch:success', {
                            uri: info.uri,
                            provider: finalResult.provider,
                            hasKaraoke: !!finalResult.karaoke,
                            hasSynced: !!finalResult.synced,
                            hasUnsynced: !!finalResult.unsynced,
                            syncDataApplied: finalResult.syncDataApplied || false
                        });

                        // IndexedDB에 캐시 저장
                        if (trackId && window.LyricsService?.cacheLyrics && !finalResult.skipCache) {
                            const cachePayload = { ...finalResult };
                            delete cachePayload.skipCache;
                            window.LyricsService.cacheLyrics(trackId, provider.id, cachePayload);
                        }

                        return finalResult;
'@
    $new = Normalize-Block @'
                        // IndexedDB에 캐시 저장
                        if (trackId && window.LyricsService?.cacheLyrics && !finalResult.skipCache) {
                            const cachePayload = { ...finalResult };
                            delete cachePayload.skipCache;
                            window.LyricsService.cacheLyrics(trackId, provider.id, cachePayload);
                        }

                        const score = scoreLyricsResult(finalResult);
                        window.__ivLyricsDebugLog?.(`[LyricsAddonManager] Candidate from ${provider.id} scored ${score}`, {
                            source: resultSource,
                            hasKaraoke,
                            hasSynced,
                            hasUnsynced,
                            currentBestScore: bestScore,
                            currentBestProvider: bestMeta?.provider || null
                        });

                        if (score > bestScore) {
                            bestScore = score;
                            bestResult = finalResult;
                            bestMeta = {
                                provider: finalResult.provider,
                                hasKaraoke,
                                hasSynced,
                                hasUnsynced,
                                syncDataApplied: finalResult.syncDataApplied || false
                            };

                            window.__ivLyricsDebugLog?.(`[LyricsAddonManager] ${provider.id} is now the best candidate`, {
                                provider: finalResult.provider,
                                score: bestScore,
                                hasKaraoke,
                                hasSynced,
                                hasUnsynced,
                                source: resultSource
                            });
                        } else {
                            window.__ivLyricsDebugLog?.(`[LyricsAddonManager] Keeping current best over ${provider.id}`, {
                                candidateScore: score,
                                bestScore,
                                candidateProvider: finalResult.provider,
                                bestProvider: bestMeta?.provider || null,
                                source: resultSource
                            });
                        }

                        const stopScore = allowKaraoke ? 3 : 2;
                        if (bestScore >= stopScore) {
                            window.__ivLyricsDebugLog?.(
                                allowKaraoke
                                    ? '[LyricsAddonManager] Karaoke result found, stopping provider search early'
                                    : '[LyricsAddonManager] Synced result found with karaoke disabled, stopping provider search early'
                            );
                            break;
                        }

                        continue;
'@

    $oldWithContentFlags = Normalize-Block @'
                        // 디버그 타이머 종료
                        if (window.AddonDebug?.isEnabled()) {
                            window.AddonDebug.timeEnd('lyrics', 'getLyrics:total');
                            window.AddonDebug.log('lyrics', 'getLyrics success', {
                                provider: finalResult.provider,
                                hasKaraoke,
                                hasSynced,
                                hasUnsynced,
                                syncDataApplied: finalResult.syncDataApplied || false
                            });
                        }

                        // 이벤트 발생
                        this.emit('lyrics:fetch:success', {
                            uri: info.uri,
                            provider: finalResult.provider,
                            hasKaraoke,
                            hasSynced,
                            hasUnsynced,
                            syncDataApplied: finalResult.syncDataApplied || false
                        });

                        // IndexedDB에 캐시 저장
                        if (trackId && window.LyricsService?.cacheLyrics && !finalResult.skipCache) {
                            const cachePayload = { ...finalResult };
                            delete cachePayload.skipCache;
                            window.LyricsService.cacheLyrics(trackId, provider.id, cachePayload);
                        }

                        return finalResult;
'@

    if ($Text.Contains($old)) {
        $Text = $Text.Replace($old, $new)
    } elseif ($Text.Contains($oldWithContentFlags)) {
        $Text = $Text.Replace($oldWithContentFlags, $new)
    } else {
        throw "Patch target block not found: replace immediate return with best candidate selection"
    }

    $anchor = Normalize-Block @'
            // 디버그 타이머 종료
            if (window.AddonDebug?.isEnabled()) {
'@
    $insertion = Normalize-Block @'
            if (bestResult) {
                window.__ivLyricsDebugLog?.('[LyricsAddonManager] Final best result selected', {
                    bestScore,
                    ...bestMeta
                });

                if (window.AddonDebug?.isEnabled()) {
                    window.AddonDebug.timeEnd('lyrics', 'getLyrics:total');
                    window.AddonDebug.log('lyrics', 'getLyrics success', bestMeta);
                }

                this.emit('lyrics:fetch:success', {
                    uri: info.uri,
                    ...bestMeta
                });

                return bestResult;
            }

'@

    return Replace-Once $Text $anchor ($insertion + $anchor) "final best result return"
}

if (-not (Test-Path $TargetPath)) {
    throw "LyricsAddonManager.js not found: $TargetPath"
}

$resolvedPath = (Resolve-Path $TargetPath).Path
$rollbackPath = $null
if (-not $WhatIf -and -not $NoApply) {
    $rollbackPath = [System.IO.Path]::GetTempFileName()
    Copy-Item -LiteralPath $resolvedPath -Destination $rollbackPath -Force
}
$original = [System.IO.File]::ReadAllText($resolvedPath)
$patched = $original
$patched = Ensure-Helpers $patched
$patched = Patch-GetLyricsState $patched
$patched = Patch-CacheSource $patched
$patched = Patch-ContentChecks $patched
$patched = Patch-ImmediateReturn $patched

if ($patched -eq $original) {
    if ($WhatIf) {
        Write-Host "Dry run: already patched: $resolvedPath"
    } else {
        Write-Host "Already patched: $resolvedPath"
    }
} elseif ($WhatIf) {
    Write-Host "Dry run: patch needed: $resolvedPath"
} else {
    $stamp = [DateTimeOffset]::UtcNow.ToString("yyyyMMddHHmmss")
    $backup = "$resolvedPath.bak-$stamp"
    [System.IO.File]::WriteAllText($backup, $original, [System.Text.UTF8Encoding]::new($false))
    [System.IO.File]::WriteAllText($resolvedPath, $patched, [System.Text.UTF8Encoding]::new($false))
    Write-Host "Patched: $resolvedPath"
    Write-Host "Backup:  $backup"
}

if ($WhatIf) {
    return
}
if ($NoApply) {
    Write-Host "Skipped spicetify apply by request."
    return
}

$spotifyWasRunning = $false
$spotifyPaths = @()

function Stop-SpotifyIfRunning {
    $processes = @(Get-Process -Name Spotify -ErrorAction SilentlyContinue)
    if ($processes.Count -eq 0) {
        return
    }

    $script:spotifyWasRunning = $true
    $script:spotifyPaths = @(
        $processes |
            ForEach-Object { $_.Path } |
            Where-Object { $_ -and (Test-Path -LiteralPath $_) } |
            Select-Object -Unique
    )

    foreach ($process in $processes) {
        Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
    }
    Start-Sleep -Seconds 2
}

function Restart-SpotifyIfNeeded {
    if (-not $script:spotifyWasRunning) {
        return
    }

    $script:spotifyWasRunning = $false
    foreach ($path in $script:spotifyPaths) {
        if ($path -and (Test-Path -LiteralPath $path)) {
            Start-Process -FilePath $path | Out-Null
            return
        }
    }

    Start-Process "spotify" -ErrorAction SilentlyContinue | Out-Null
}

if (Get-Command spicetify -ErrorAction SilentlyContinue) {
    try {
        try {
            Stop-SpotifyIfRunning
            spicetify apply
            if ($LASTEXITCODE -ne 0) {
                throw "spicetify apply failed with exit code $LASTEXITCODE"
            }
        } catch {
            if ($rollbackPath -and (Test-Path -LiteralPath $rollbackPath)) {
                Copy-Item -LiteralPath $rollbackPath -Destination $resolvedPath -Force -ErrorAction SilentlyContinue
                Write-Warning "Restored LyricsAddonManager.js because spicetify apply failed."
            }
            throw
        }
    } finally {
        Restart-SpotifyIfNeeded
        if ($rollbackPath -and (Test-Path -LiteralPath $rollbackPath)) {
            Remove-Item -LiteralPath $rollbackPath -Force -ErrorAction SilentlyContinue
        }
    }
} else {
    Write-Warning "spicetify not found; patch was written but apply was skipped."
    if ($rollbackPath -and (Test-Path -LiteralPath $rollbackPath)) {
        Remove-Item -LiteralPath $rollbackPath -Force -ErrorAction SilentlyContinue
    }
}
