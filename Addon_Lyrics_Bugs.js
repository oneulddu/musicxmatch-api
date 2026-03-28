/**
 * Generated file. Do not edit directly.
 * @addon-type  lyrics
 * @id          bugs-provider
 * @name        Bugs Provider
 * @version     0.7.8
 * @author      oneulddu
 * @generated   scripts/generate_addons.js
 */

(() => {
    'use strict';

    const PROVIDER = {
    "id": "bugs-provider",
    "name": "Bugs Provider",
    "backend": "bugs",
    "version": "0.7.8",
    "author": "oneulddu",
    "description": "Fetches synced or plain lyrics from Bugs through the local bridge server.",
    "settingsTitle": "Lyrics bridge server",
    "settingsHint": "Run the local lyrics server and point this addon to it.",
    "icon": "M12 3v10.55A4 4 0 1 0 14 17V7h4V3h-6z",
    "supports": {
        "karaoke": false,
        "synced": true,
        "unsynced": true
    }
};
    const SERVER_CONFIG = null;
    const DEFAULT_SERVER_URL = "http://127.0.0.1:8092";
    const DEFAULT_TIMEOUT_SEC = 15;

    const ADDON_INFO = {
        id: PROVIDER.id,
        name: PROVIDER.name,
        author: PROVIDER.author,
        version: PROVIDER.version,
        description: {
            en: PROVIDER.description,
        },
        supports: PROVIDER.supports,
        useIvLyricsSync: true,
        icon: PROVIDER.icon,
    };

    const SETTING = {
        SERVER_URL: 'server-url',
        TIMEOUT_SEC: 'timeout-sec',
    };

    function getSetting(key, defaultValue) {
        return window.LyricsAddonManager?.getAddonSetting(PROVIDER.id, key, defaultValue) ?? defaultValue;
    }

    function setSetting(key, value) {
        window.LyricsAddonManager?.setAddonSetting(PROVIDER.id, key, value);
    }

    function normalizeServerUrl(value) {
        return (value || DEFAULT_SERVER_URL).replace(/\/$/, '');
    }

    function getServerUrl() {
        return normalizeServerUrl(getSetting(SETTING.SERVER_URL, DEFAULT_SERVER_URL));
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
            // Let fetch surface invalid URLs.
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

    function parseVersion(value) {
        return String(value || '0.0.0')
            .split('.')
            .map((part) => parseInt(part, 10) || 0);
    }

    function compareVersions(left, right) {
        const a = parseVersion(left);
        const b = parseVersion(right);
        const length = Math.max(a.length, b.length);
        for (let index = 0; index < length; index += 1) {
            const delta = (a[index] || 0) - (b[index] || 0);
            if (delta !== 0) {
                return delta;
            }
        }
        return 0;
    }

    async function parseErrorResponse(response) {
        let detail = `HTTP ${response.status}`;
        try {
            const payload = await response.json();
            detail = payload.detail || detail;
        } catch {
            // Keep generic status text.
        }
        return detail;
    }

    async function fetchVersionStatus(serverUrl) {
        const versionState = {
            latestAddonVersion: null,
            latestServerVersion: null,
            currentAddonVersion: ADDON_INFO.version,
            currentServerVersion: null,
            addonOutdated: false,
            serverOutdated: false,
            serverCommand: [],
            allCommand: [],
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
                versionState.addonOutdated = versionState.latestAddonVersion
                    ? compareVersions(versionState.latestAddonVersion, ADDON_INFO.version) > 0
                    : false;
                versionState.serverCommand = Array.isArray(payload.serverCommand) ? payload.serverCommand : [];
                versionState.allCommand = Array.isArray(payload.allCommand) ? payload.allCommand : [];
            }
        } catch (error) {
            versionState.error = error.message;
        }

        return versionState;
    }

    async function fetchServerConfig(serverUrl) {
        if (!SERVER_CONFIG) {
            return null;
        }

        const configState = {
            configured: false,
            preview: null,
            error: null,
        };

        try {
            const { response } = await fetchJsonWithFallback(serverUrl || DEFAULT_SERVER_URL, '/config', 5000);
            if (!response.ok) {
                configState.error = `HTTP ${response.status}`;
                return configState;
            }

            const payload = await response.json();
            if (SERVER_CONFIG.kind === 'deezerArl') {
                configState.configured = !!payload.deezerArlConfigured;
                configState.preview = payload.deezerArlPreview || null;
            }
        } catch (error) {
            configState.error = error.message;
        }

        return configState;
    }

    async function saveServerConfig(serverUrl, rawValue) {
        if (!SERVER_CONFIG) {
            return null;
        }

        const payload = {};
        if (SERVER_CONFIG.kind === 'deezerArl') {
            payload.deezerArl = rawValue.trim() || '';
        }

        const { response } = await fetchJsonWithFallback(serverUrl || DEFAULT_SERVER_URL, '/config', 10000, {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify(payload),
        });

        if (!response.ok) {
            throw new Error(await parseErrorResponse(response));
        }

        return response.json();
    }

    function getSettingsUI() {
        const React = Spicetify.React;
        const { useEffect, useState } = React;

        return function ProviderSettings() {
            const [serverUrl, setServerUrl] = useState(() => getSetting(SETTING.SERVER_URL, DEFAULT_SERVER_URL));
            const [timeoutSec, setTimeoutSec] = useState(() => getSetting(SETTING.TIMEOUT_SEC, DEFAULT_TIMEOUT_SEC));
            const [status, setStatus] = useState(null);
            const [versionStatus, setVersionStatus] = useState(null);
            const [updateStatus, setUpdateStatus] = useState(null);
            const [updateAllStatus, setUpdateAllStatus] = useState(null);
            const [serverConfigValue, setServerConfigValue] = useState('');
            const [serverConfigState, setServerConfigState] = useState(null);
            const [serverConfigStatus, setServerConfigStatus] = useState(null);

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

            const runUpdateAll = async () => {
                setUpdateAllStatus('updating');
                try {
                    const { response } = await fetchJsonWithFallback(serverUrl || DEFAULT_SERVER_URL, '/update/apply-all', 10000, {
                        method: 'POST',
                    });
                    setUpdateAllStatus(response.ok ? 'scheduled' : 'failed');
                } catch {
                    setUpdateAllStatus('failed');
                }
            };

            const saveAdditionalConfig = async (value) => {
                if (!SERVER_CONFIG) {
                    return;
                }
                setServerConfigStatus('saving');
                try {
                    const payload = await saveServerConfig(serverUrl || DEFAULT_SERVER_URL, value);
                    if (SERVER_CONFIG.kind === 'deezerArl') {
                        setServerConfigState({
                            configured: !!payload.deezerArlConfigured,
                            preview: payload.deezerArlPreview || null,
                            error: null,
                        });
                    }
                    setServerConfigValue('');
                    setServerConfigStatus(value.trim() ? 'saved' : 'cleared');
                } catch (error) {
                    setServerConfigStatus(`failed:${error.message}`);
                }
            };

            useEffect(() => {
                let cancelled = false;

                fetchVersionStatus(serverUrl || DEFAULT_SERVER_URL).then((nextStatus) => {
                    if (!cancelled) {
                        setVersionStatus(nextStatus);
                    }
                });

                if (SERVER_CONFIG) {
                    fetchServerConfig(serverUrl || DEFAULT_SERVER_URL).then((nextState) => {
                        if (!cancelled) {
                            setServerConfigState(nextState);
                        }
                    });
                }

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
            const subtleButton = {
                ...button,
                background: 'rgba(255,255,255,0.16)',
                color: '#fff',
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
                React.createElement('div', { style: { fontSize: 12, fontWeight: 700, marginBottom: 8 } }, PROVIDER.settingsTitle),
                React.createElement('input', {
                    type: 'text',
                    value: serverUrl,
                    style: input,
                    placeholder: DEFAULT_SERVER_URL,
                    onChange: (event) => saveUrl(event.target.value),
                }),
                React.createElement('div', { style: { fontSize: 12, opacity: 0.7, marginTop: 8 } }, PROVIDER.settingsHint),
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
                SERVER_CONFIG && React.createElement(React.Fragment, null,
                    React.createElement('div', { style: { fontSize: 12, fontWeight: 700, marginTop: 18, marginBottom: 8 } }, SERVER_CONFIG.title),
                    React.createElement('input', {
                        type: 'password',
                        value: serverConfigValue,
                        style: input,
                        placeholder: SERVER_CONFIG.placeholder,
                        onChange: (event) => {
                            setServerConfigValue(event.target.value);
                            setServerConfigStatus(null);
                        },
                    }),
                    React.createElement('div', { style: { fontSize: 12, opacity: 0.7, marginTop: 8 } }, SERVER_CONFIG.hint),
                    React.createElement('div', { style: { display: 'flex', alignItems: 'center', gap: 10, marginTop: 10 } },
                        React.createElement('button', {
                            style: button,
                            onClick: () => saveAdditionalConfig(serverConfigValue),
                            disabled: serverConfigStatus === 'saving',
                        }, serverConfigStatus === 'saving' ? 'Saving...' : SERVER_CONFIG.saveLabel),
                        React.createElement('button', {
                            style: subtleButton,
                            onClick: () => saveAdditionalConfig(''),
                            disabled: serverConfigStatus === 'saving',
                        }, 'Clear'),
                        serverConfigState?.configured && React.createElement('span', { style: { color: '#1db954', fontSize: 12, fontWeight: 700 } }, SERVER_CONFIG.configuredLabel),
                        serverConfigStatus === 'saved' && React.createElement('span', { style: { color: '#1db954', fontSize: 12, fontWeight: 700 } }, SERVER_CONFIG.savedLabel),
                        serverConfigStatus === 'cleared' && React.createElement('span', { style: { color: '#f59e0b', fontSize: 12, fontWeight: 700 } }, SERVER_CONFIG.clearedLabel),
                        serverConfigStatus?.startsWith('failed:') && React.createElement('span', { style: { color: '#e91429', fontSize: 12, fontWeight: 700 } }, 'Save failed')
                    ),
                    serverConfigState?.preview && React.createElement('div', { style: { fontSize: 12, opacity: 0.7, marginTop: 8 } },
                        `${SERVER_CONFIG.previewLabel}: ${serverConfigState.preview}`
                    ),
                    serverConfigState?.error && React.createElement('div', { style: { color: '#e91429', fontSize: 12, marginTop: 8 } },
                        `${SERVER_CONFIG.errorPrefix}: ${serverConfigState.error}`
                    )
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
                    React.createElement('div', { style: { fontSize: 12, opacity: 0.8 } }, 'Update server refreshes only the local bridge server. Update all refreshes the server and provider addons together.'),
                    React.createElement('div', { style: { display: 'flex', alignItems: 'center', gap: 10, marginTop: 10 } },
                        React.createElement('button', {
                            style: button,
                            onClick: runUpdate,
                            disabled: updateStatus === 'updating',
                        }, updateStatus === 'updating' ? 'Updating server...' : 'Update server'),
                        React.createElement('button', {
                            style: button,
                            onClick: runUpdateAll,
                            disabled: updateAllStatus === 'updating',
                        }, updateAllStatus === 'updating' ? 'Updating all...' : 'Update all'),
                        updateStatus === 'scheduled' && React.createElement('span', { style: { color: '#1db954', fontSize: 12, fontWeight: 700 } }, 'Server update scheduled'),
                        updateStatus === 'failed' && React.createElement('span', { style: { color: '#e91429', fontSize: 12, fontWeight: 700 } }, 'Update server failed'),
                        updateAllStatus === 'scheduled' && React.createElement('span', { style: { color: '#1db954', fontSize: 12, fontWeight: 700 } }, 'All updates scheduled'),
                        updateAllStatus === 'failed' && React.createElement('span', { style: { color: '#e91429', fontSize: 12, fontWeight: 700 } }, 'Update all failed')
                    ),
                    React.createElement('div', { style: { fontSize: 12, opacity: 0.75, marginTop: 10 } }, 'Server only'),
                    React.createElement('div', { style: commandBox }, (versionStatus.serverCommand || []).join('\n')),
                    React.createElement('div', { style: { fontSize: 12, opacity: 0.75, marginTop: 10 } }, 'Update all'),
                    React.createElement('div', { style: commandBox }, (versionStatus.allCommand || []).join('\n'))
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
            provider: PROVIDER.id,
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
        params.set('backend', PROVIDER.backend);
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
            result.error = await parseErrorResponse(response);
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

    const addon = {
        ...ADDON_INFO,
        async init() {
            window.__ivLyricsDebugLog?.(`[${PROVIDER.id}] initialized`);
        },
        getSettingsUI,
        getLyrics,
    };

    const register = () => {
        if (window.LyricsAddonManager) {
            window.LyricsAddonManager.register(addon);
            return;
        }
        setTimeout(register, 100);
    };

    register();
})();
