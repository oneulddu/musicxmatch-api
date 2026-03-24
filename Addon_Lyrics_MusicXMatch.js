/**
 * @addon-type  lyrics
 * @id          musicxmatch-provider
 * @name        MusicXMatch Provider
 * @version     0.2.1
 * @author      oneulddu
 */

(() => {
    'use strict';

    const ADDON_ID = 'musicxmatch-provider';
    const DEFAULT_SERVER_URL = 'http://127.0.0.1:8092';
    const DEFAULT_TIMEOUT_SEC = 15;

    const ADDON_INFO = {
        id: ADDON_ID,
        name: 'MusicXMatch Provider',
        author: 'oneulddu',
        version: '0.3.0',
        description: {
            en: 'Fetches synced or plain lyrics from a local MusicXMatch bridge server.',
        },
        supports: {
            karaoke: false,
            synced: true,
            unsynced: true,
        },
        useIvLyricsSync: true,
        icon: 'M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6z',
    };

    const SETTING = {
        SERVER_URL: 'server-url',
        TIMEOUT_SEC: 'timeout-sec',
    };

    function getSetting(key, defaultValue) {
        return window.LyricsAddonManager?.getAddonSetting(ADDON_ID, key, defaultValue) ?? defaultValue;
    }

    function setSetting(key, value) {
        window.LyricsAddonManager?.setAddonSetting(ADDON_ID, key, value);
    }

    function normalizeServerUrl(value) {
        return (value || DEFAULT_SERVER_URL).replace(/\/$/, '');
    }

    function getServerUrl() {
        const value = getSetting(SETTING.SERVER_URL, DEFAULT_SERVER_URL);
        return normalizeServerUrl(value);
    }

    function getServerCandidates(serverUrl) {
        const normalized = normalizeServerUrl(serverUrl);
        const candidates = [normalized];

        try {
            const parsed = new URL(normalized);
            if (parsed.hostname === 'localhost') {
                parsed.hostname = '127.0.0.1';
                candidates.push(parsed.toString().replace(/\/$/, ''));
            }
        } catch {
            // Ignore invalid URLs here and let fetch surface the error later.
        }

        return [...new Set(candidates)];
    }

    async function fetchJsonWithFallback(serverUrl, path, timeoutMs, init = {}) {
        const candidates = getServerCandidates(serverUrl);
        let lastError = null;

        for (const baseUrl of candidates) {
            try {
                const response = await fetch(`${baseUrl}${path}`, {
                    ...init,
                    signal: AbortSignal.timeout(timeoutMs),
                });
                return { response, baseUrl };
            } catch (error) {
                lastError = error;
            }
        }

        throw lastError || new Error('Request failed');
    }

    function getTimeoutMs() {
        const value = parseInt(getSetting(SETTING.TIMEOUT_SEC, DEFAULT_TIMEOUT_SEC), 10);
        return (Number.isNaN(value) || value < 5 ? DEFAULT_TIMEOUT_SEC : value) * 1000;
    }

    function parseLrc(lrc) {
        if (!lrc || typeof lrc !== 'string') {
            return { synced: null, unsynced: null };
        }

        const synced = [];
        const unsynced = [];
        for (const line of lrc.split('\n')) {
            const match = line.match(/\[(\d+):(\d+)(?:[.,](\d+))?\](.*)/);
            if (!match) {
                continue;
            }

            const minutes = parseInt(match[1], 10);
            const seconds = parseInt(match[2], 10);
            const fraction = parseInt((match[3] || '0').padEnd(2, '0').slice(0, 2), 10);
            const text = match[4].trim();
            if (!text) {
                continue;
            }

            synced.push({
                startTime: minutes * 60 * 1000 + seconds * 1000 + fraction * 10,
                text,
            });
            unsynced.push({ text });
        }

        return {
            synced: synced.length ? synced : null,
            unsynced: unsynced.length ? unsynced : null,
        };
    }

    function parsePlainLyrics(text) {
        if (!text || typeof text !== 'string') {
            return null;
        }

        const lines = text
            .split('\n')
            .map((line) => line.trim())
            .filter(Boolean)
            .map((line) => ({ text: line }));

        return lines.length ? lines : null;
    }

    async function fetchVersionStatus(serverUrl) {
        const versionState = {
            latestAddonVersion: null,
            latestServerVersion: null,
            currentAddonVersion: ADDON_INFO.version,
            currentServerVersion: null,
            addonOutdated: false,
            serverOutdated: false,
            command: [],
            error: null,
        };

        try {
            const { response } = await fetchJsonWithFallback(serverUrl || DEFAULT_SERVER_URL, '/update/check', 5000);
            if (response.ok) {
                const payload = await response.json();
                versionState.currentServerVersion = payload.currentVersion || null;
                versionState.latestServerVersion = payload.latestVersion || null;
                versionState.latestAddonVersion = payload.latestAddonVersion || payload.latestVersion || null;
                versionState.serverOutdated = !!payload.updateAvailable;
                versionState.addonOutdated = false;
                versionState.command = Array.isArray(payload.command) ? payload.command : [];
            }
        } catch (error) {
            versionState.error = error.message;
            return versionState;
        }

        return versionState;
    }

    function getSettingsUI() {
        const React = Spicetify.React;
        const { useEffect, useState } = React;

        return function MusicXMatchSettings() {
            const [serverUrl, setServerUrl] = useState(() => getSetting(SETTING.SERVER_URL, DEFAULT_SERVER_URL));
            const [timeoutSec, setTimeoutSec] = useState(() => getSetting(SETTING.TIMEOUT_SEC, DEFAULT_TIMEOUT_SEC));
            const [status, setStatus] = useState(null);
            const [versionStatus, setVersionStatus] = useState(null);
            const [updateStatus, setUpdateStatus] = useState(null);

            const saveUrl = (value) => {
                setServerUrl(value);
                setSetting(SETTING.SERVER_URL, value);
                setStatus(null);
            };

            const saveTimeout = (value) => {
                setTimeoutSec(value);
                setSetting(SETTING.TIMEOUT_SEC, value);
            };

            const testConnection = async () => {
                setStatus('testing');
                try {
                    const { response, baseUrl } = await fetchJsonWithFallback(serverUrl || DEFAULT_SERVER_URL, '/health', 5000);
                    if (baseUrl !== normalizeServerUrl(serverUrl || DEFAULT_SERVER_URL)) {
                        saveUrl(baseUrl);
                    }
                    setStatus(response.ok ? 'ok' : 'fail');
                } catch {
                    setStatus('fail');
                }
            };

            const runUpdate = async () => {
                setUpdateStatus('updating');
                try {
                    const { response } = await fetchJsonWithFallback(serverUrl || DEFAULT_SERVER_URL, '/update/apply', 10000, {
                        method: 'POST',
                    });
                    setUpdateStatus(response.ok ? 'scheduled' : 'failed');
                } catch {
                    setUpdateStatus('failed');
                }
            };

            useEffect(() => {
                let cancelled = false;

                fetchVersionStatus(serverUrl || DEFAULT_SERVER_URL).then((result) => {
                    if (!cancelled) {
                        setVersionStatus(result);
                    }
                });

                return () => {
                    cancelled = true;
                };
            }, [serverUrl]);

            const box = {
                background: 'rgba(255,255,255,0.05)',
                border: '1px solid rgba(255,255,255,0.08)',
                borderRadius: 10,
                padding: '14px 16px',
            };
            const input = {
                width: '100%',
                background: 'rgba(255,255,255,0.08)',
                border: '1px solid rgba(255,255,255,0.12)',
                borderRadius: 6,
                color: 'inherit',
                padding: '8px 10px',
                boxSizing: 'border-box',
            };
            const button = {
                padding: '8px 12px',
                borderRadius: 6,
                border: 'none',
                cursor: 'pointer',
                background: '#1db954',
                color: '#000',
                fontWeight: 700,
            };
            const commandBox = {
                marginTop: 14,
                padding: '10px 12px',
                borderRadius: 8,
                background: 'rgba(0,0,0,0.25)',
                border: '1px solid rgba(255,255,255,0.08)',
                fontSize: 12,
                fontFamily: 'ui-monospace, SFMono-Regular, Menlo, monospace',
                whiteSpace: 'pre-wrap',
                wordBreak: 'break-word',
            };
            const updateNeeded = !!(versionStatus && (versionStatus.addonOutdated || versionStatus.serverOutdated));

            return React.createElement('div', { style: box },
                React.createElement('div', { style: { fontSize: 12, fontWeight: 700, marginBottom: 8 } }, 'MusicXMatch server'),
                React.createElement('input', {
                    type: 'text',
                    value: serverUrl,
                    style: input,
                    placeholder: DEFAULT_SERVER_URL,
                    onChange: (event) => saveUrl(event.target.value),
                }),
                React.createElement('div', { style: { fontSize: 12, opacity: 0.7, marginTop: 8 } }, 'Run the local lyrics server and point this addon to it.'),
                React.createElement('div', { style: { fontSize: 12, fontWeight: 700, marginTop: 14, marginBottom: 6 } }, `Timeout: ${timeoutSec}s`),
                React.createElement('input', {
                    type: 'range',
                    min: '5',
                    max: '60',
                    step: '5',
                    value: String(timeoutSec),
                    style: { width: '100%' },
                    onChange: (event) => saveTimeout(Number(event.target.value)),
                }),
                React.createElement('div', { style: { display: 'flex', alignItems: 'center', gap: 10, marginTop: 14 } },
                    React.createElement('button', {
                        style: button,
                        onClick: testConnection,
                        disabled: status === 'testing',
                    }, status === 'testing' ? 'Testing...' : 'Test connection'),
                    status === 'ok' && React.createElement('span', { style: { color: '#1db954', fontSize: 12, fontWeight: 700 } }, 'Connected'),
                    status === 'fail' && React.createElement('span', { style: { color: '#e91429', fontSize: 12, fontWeight: 700 } }, 'Failed')
                ),
                React.createElement('div', { style: { fontSize: 12, opacity: 0.8, marginTop: 14 } },
                    `Addon: ${ADDON_INFO.version}`,
                    versionStatus?.currentServerVersion ? ` | Server: ${versionStatus.currentServerVersion}` : ''
                ),
                versionStatus?.latestAddonVersion && React.createElement('div', { style: { fontSize: 12, opacity: 0.7, marginTop: 6 } },
                    `Latest addon: ${versionStatus.latestAddonVersion}`,
                    versionStatus.latestServerVersion ? ` | Latest server: ${versionStatus.latestServerVersion}` : ''
                ),
                updateNeeded && React.createElement('div', { style: { marginTop: 14 } },
                    React.createElement('div', { style: { color: '#f59e0b', fontSize: 12, fontWeight: 700, marginBottom: 8 } }, 'Update available'),
                    React.createElement('div', { style: { fontSize: 12, opacity: 0.8 } }, 'Run the commands below to update the server and addon:'),
                    React.createElement('div', { style: { display: 'flex', alignItems: 'center', gap: 10, marginTop: 10 } },
                        React.createElement('button', {
                            style: button,
                            onClick: runUpdate,
                            disabled: updateStatus === 'updating',
                        }, updateStatus === 'updating' ? 'Updating...' : 'Update now'),
                        updateStatus === 'scheduled' && React.createElement('span', { style: { color: '#1db954', fontSize: 12, fontWeight: 700 } }, 'Scheduled'),
                        updateStatus === 'failed' && React.createElement('span', { style: { color: '#e91429', fontSize: 12, fontWeight: 700 } }, 'Failed')
                    ),
                    React.createElement('div', { style: commandBox }, (versionStatus.command || []).join('\n'))
                ),
                versionStatus?.error && React.createElement('div', { style: { color: '#e91429', fontSize: 12, marginTop: 14 } },
                    `Version check failed: ${versionStatus.error}`
                )
            );
        };
    }

    async function getLyrics(info) {
        const result = {
            uri: info.uri,
            provider: ADDON_ID,
            karaoke: null,
            synced: null,
            unsynced: null,
            copyright: null,
            error: null,
        };

        const title = (info.title || '').trim();
        const artist = (info.artist || '').trim();
        if (!title || !artist) {
            result.error = 'Track title and artist are required.';
            return result;
        }

        const serverUrl = getServerUrl();
        const timeout = getTimeoutMs();
        const spotifyId = typeof info.uri === 'string' && info.uri.startsWith('spotify:track:')
            ? info.uri.split(':')[2]
            : '';
        const params = new URLSearchParams({ title, artist });
        if (spotifyId) {
            params.set('spotifyId', spotifyId);
        }
        if (typeof info.duration === 'number' && Number.isFinite(info.duration) && info.duration > 0) {
            params.set('durationMs', String(Math.round(info.duration)));
        }

        let response;
        try {
            const fetchResult = await fetchJsonWithFallback(serverUrl, `/lyrics?${params.toString()}`, timeout);
            response = fetchResult.response;
            if (fetchResult.baseUrl !== serverUrl) {
                setSetting(SETTING.SERVER_URL, fetchResult.baseUrl);
            }
        } catch (error) {
            result.error = `Server connection failed: ${error.message}`;
            return result;
        }

        if (!response.ok) {
            try {
                const payload = await response.json();
                result.error = payload.detail || `HTTP ${response.status}`;
            } catch {
                result.error = `HTTP ${response.status}`;
            }
            return result;
        }

        let payload;
        try {
            payload = await response.json();
        } catch {
            result.error = 'Could not parse server response.';
            return result;
        }

        if (payload.lrc) {
            const parsed = parseLrc(payload.lrc);
            result.synced = parsed.synced;
            result.unsynced = parsed.unsynced;
        } else if (payload.text) {
            result.unsynced = parsePlainLyrics(payload.text);
        }

        if (!result.synced && !result.unsynced) {
            result.error = 'No usable lyrics were returned.';
        }

        return result;
    }

    const MusicXMatchAddon = {
        ...ADDON_INFO,
        async init() {
            window.__ivLyricsDebugLog?.(`[${ADDON_ID}] initialized`);
        },
        getSettingsUI,
        getLyrics,
    };

    const register = () => {
        if (window.LyricsAddonManager) {
            window.LyricsAddonManager.register(MusicXMatchAddon);
            return;
        }
        setTimeout(register, 100);
    };

    register();
})();
