/**
 * MusicXMatch API Client
 * Ported from Python implementation
 */

'use strict';

const crypto = require('crypto');
const https = require('https');

const USER_AGENT = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/113.0.0.0 Safari/537.36';

class MusicXMatchAPI {
    constructor() {
        this.baseUrl = 'https://www.musixmatch.com/ws/1.1/';
        this.secret = null;
    }

    async init() {
        if (!this.secret) {
            this.secret = await this._getSecret();
        }
    }

    async _getLatestApp() {
        const html = await this._fetch('https://www.musixmatch.com/search');
        const match = html.match(/src="([^"]*\/_next\/static\/chunks\/pages\/_app-[^"]+\.js)"/);
        if (!match) throw new Error('_app URL not found');
        return match[1];
    }

    async _getSecret() {
        const appUrl = await this._getLatestApp();
        const js = await this._fetch(appUrl);
        const match = js.match(/from\(\s*"(.*?)"\s*\.split/);
        if (!match) throw new Error('Secret not found');

        const reversed = match[1].split('').reverse().join('');
        return Buffer.from(reversed, 'base64').toString('utf-8');
    }

    _generateSignature(url) {
        const now = new Date();
        const y = now.getFullYear().toString();
        const m = (now.getMonth() + 1).toString().padStart(2, '0');
        const d = now.getDate().toString().padStart(2, '0');

        const message = url + y + m + d;
        const hmac = crypto.createHmac('sha256', this.secret);
        hmac.update(message);
        const sig = hmac.digest('base64');

        return `&signature=${encodeURIComponent(sig)}&signature_protocol=sha256`;
    }

    async searchTracks(query, page = 1) {
        const url = `track.search?app_id=web-desktop-app-v1.0&format=json&q=${encodeURIComponent(query)}&f_has_lyrics=true&page_size=100&page=${page}`;
        return this._makeRequest(url);
    }

    async getTrackLyrics(trackId) {
        const url = `track.lyrics.get?app_id=web-desktop-app-v1.0&format=json&track_id=${trackId}`;
        return this._makeRequest(url);
    }

    async getTrackRichsync(trackId) {
        const url = `track.richsync.get?app_id=web-desktop-app-v1.0&format=json&track_id=${trackId}`;
        return this._makeRequest(url);
    }

    async _makeRequest(path) {
        const url = this.baseUrl + path.replace(/%20/g, '+').replace(/ /g, '+');
        const signedUrl = url + this._generateSignature(url);
        const data = await this._fetch(signedUrl);
        return JSON.parse(data);
    }

    _fetch(url) {
        return new Promise((resolve, reject) => {
            https.get(url, { headers: { 'User-Agent': USER_AGENT } }, (res) => {
                let data = '';
                res.on('data', chunk => data += chunk);
                res.on('end', () => resolve(data));
            }).on('error', reject).setTimeout(10000, function() { this.destroy(); });
        });
    }
}

module.exports = MusicXMatchAPI;
