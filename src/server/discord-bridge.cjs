const net = require('net');
const https = require('https');

const CLIENT_ID = "1563832553541075005";
const CLIENT_SECRET = "yzHU5CDNpkdWdDeiUDynBTEHnsNual82";
const PIPE_NAME = '\\\\.\\pipe\\discord-ipc-0';

class DiscordBridge {
  constructor() {
    this.client = null;
    this.accessToken = null;
    this.reconnectTimer = null;
    this.subscribers = new Set();
    this.speakingUsers = new Set();
    this.channelId = null;

    this.state = {
      connected: false,
      inVoice: false,
      channelId: null,
      channelName: "",
      guildId: null,
      selfMuted: false,
      selfDeafened: false,
      users: []
    };

    this.connect();
  }

  notifySubscribers() {
    for (const res of this.subscribers) {
      try {
        res.write(`data: ${JSON.stringify(this.state)}\n\n`);
      } catch (e) {
        this.subscribers.delete(res);
      }
    }
  }

  addSubscriber(res) {
    this.subscribers.add(res);
    res.write(`data: ${JSON.stringify(this.state)}\n\n`);
  }

  removeSubscriber(res) {
    this.subscribers.delete(res);
  }

  connect() {
    if (this.client) {
      try { this.client.destroy(); } catch (e) {}
    }

    console.log("[DiscordBridge] Connecting to Discord IPC...");
    this.client = net.createConnection(PIPE_NAME, () => {
      console.log("[DiscordBridge] Connected to Discord pipe!");
      this.sendHandshake();
    });

    let buffer = Buffer.alloc(0);

    this.client.on('data', async (chunk) => {
      buffer = Buffer.concat([buffer, chunk]);

      while (buffer.length >= 8) {
        const op = buffer.readUInt32LE(0);
        const len = buffer.readUInt32LE(4);

        if (buffer.length < 8 + len) {
          break; // wait for full frame
        }

        const bodyStr = buffer.slice(8, 8 + len).toString('utf8');
        buffer = buffer.slice(8 + len);

        try {
          const msg = JSON.parse(bodyStr);
          await this.handleMessage(msg);
        } catch (e) {
          console.error("[DiscordBridge] Parse error:", e);
        }
      }
    });

    this.client.on('error', (err) => {
      console.log("[DiscordBridge] Pipe error:", err.message);
    });

    this.client.on('close', () => {
      console.log("[DiscordBridge] Pipe closed. Reconnecting in 3s...");
      this.client = null;
      this.state.connected = false;
      this.state.inVoice = false;
      this.notifySubscribers();
      clearTimeout(this.reconnectTimer);
      this.reconnectTimer = setTimeout(() => this.connect(), 3000);
    });
  }

  sendHandshake() {
    const payload = JSON.stringify({ v: 1, client_id: CLIENT_ID });
    const payloadBuf = Buffer.from(payload, 'utf8');
    const header = Buffer.alloc(8);
    header.writeUInt32LE(0, 0); // opcode 0 = HANDSHAKE
    header.writeUInt32LE(payloadBuf.length, 4);
    this.client.write(Buffer.concat([header, payloadBuf]));
  }

  sendFrame(cmd, args = {}, nonce = Math.random().toString(36).substring(7)) {
    if (!this.client || this.client.destroyed) return;
    const payload = JSON.stringify({ cmd, args, nonce });
    const buf = Buffer.from(payload, 'utf8');
    const hdr = Buffer.alloc(8);
    hdr.writeUInt32LE(1, 0); // opcode 1 = FRAME
    hdr.writeUInt32LE(buf.length, 4);
    this.client.write(Buffer.concat([hdr, buf]));
  }

  subscribe(evt, args = {}, nonce = Math.random().toString(36).substring(7)) {
    if (!this.client || this.client.destroyed) return;
    const payload = JSON.stringify({ cmd: "SUBSCRIBE", evt, args, nonce });
    const buf = Buffer.from(payload, 'utf8');
    const hdr = Buffer.alloc(8);
    hdr.writeUInt32LE(1, 0);
    hdr.writeUInt32LE(buf.length, 4);
    this.client.write(Buffer.concat([hdr, buf]));
  }

  async exchangeToken(code) {
    return new Promise((resolve, reject) => {
      const postData = new URLSearchParams({
        client_id: CLIENT_ID,
        client_secret: CLIENT_SECRET,
        grant_type: 'authorization_code',
        code: code
      }).toString();

      const req = https.request('https://discord.com/api/v10/oauth2/token', {
        method: 'POST',
        headers: {
          'Content-Type': 'application/x-www-form-urlencoded',
          'Content-Length': Buffer.byteLength(postData)
        }
      }, (res) => {
        let data = '';
        res.on('data', chunk => data += chunk);
        res.on('end', () => {
          try {
            resolve(JSON.parse(data));
          } catch (e) {
            reject(e);
          }
        });
      });

      req.on('error', reject);
      req.write(postData);
      req.end();
    });
  }

  async handleMessage(msg) {
    if (msg.cmd === "DISPATCH" && msg.evt === "READY") {
      this.state.connected = true;
      if (this.accessToken) {
        this.sendFrame("AUTHENTICATE", { access_token: this.accessToken });
      } else {
        this.sendFrame("AUTHORIZE", { client_id: CLIENT_ID, scopes: ["rpc", "identify"] });
      }
    } else if (msg.cmd === "AUTHORIZE") {
      if (msg.data && msg.data.code) {
        const tokenResp = await this.exchangeToken(msg.data.code);
        if (tokenResp && tokenResp.access_token) {
          this.accessToken = tokenResp.access_token;
          this.sendFrame("AUTHENTICATE", { access_token: this.accessToken });
        }
      }
    } else if (msg.cmd === "AUTHENTICATE") {
      console.log("[DiscordBridge] Authenticated with Discord!");
      this.subscribe("VOICE_CHANNEL_SELECT");
      this.subscribe("VOICE_SETTINGS_UPDATE");
      this.sendFrame("GET_VOICE_SETTINGS");
      this.sendFrame("GET_SELECTED_VOICE_CHANNEL");
    } else if (msg.cmd === "GET_VOICE_SETTINGS") {
      if (msg.data) {
        this.state.selfMuted = !!msg.data.mute;
        this.state.selfDeafened = !!msg.data.deaf;
        this.notifySubscribers();
      }
    } else if (msg.evt === "VOICE_SETTINGS_UPDATE") {
      if (msg.data) {
        this.state.selfMuted = !!msg.data.mute;
        this.state.selfDeafened = !!msg.data.deaf;
        this.notifySubscribers();
      }
    } else if (msg.evt === "VOICE_CHANNEL_SELECT") {
      console.log("[DiscordBridge] VOICE_CHANNEL_SELECT event:", msg.data);
      if (msg.data && msg.data.channel_id) {
        this.channelId = msg.data.channel_id;
        this.subscribe("SPEAKING_START", { channel_id: msg.data.channel_id });
        this.subscribe("SPEAKING_STOP", { channel_id: msg.data.channel_id });
        this.subscribe("VOICE_STATE_CREATE", { channel_id: msg.data.channel_id });
        this.subscribe("VOICE_STATE_DELETE", { channel_id: msg.data.channel_id });
        this.subscribe("VOICE_STATE_UPDATE", { channel_id: msg.data.channel_id });
        this.sendFrame("GET_SELECTED_VOICE_CHANNEL");
      } else {
        this.channelId = null;
        this.state.inVoice = false;
        this.state.channelId = null;
        this.state.channelName = "";
        this.state.users = [];
        this.speakingUsers.clear();
        this.notifySubscribers();
      }
    } else if (msg.cmd === "GET_SELECTED_VOICE_CHANNEL") {
      this.updateVoiceChannel(msg.data);
    } else if (msg.evt === "VOICE_STATE_CREATE" || msg.evt === "VOICE_STATE_DELETE" || msg.evt === "VOICE_STATE_UPDATE") {
      this.sendFrame("GET_SELECTED_VOICE_CHANNEL");
    } else if (msg.evt === "SPEAKING_START") {
      if (msg.data && msg.data.user_id) {
        this.speakingUsers.add(msg.data.user_id);
        this.updateSpeakingFlags();
      }
    } else if (msg.evt === "SPEAKING_STOP") {
      if (msg.data && msg.data.user_id) {
        this.speakingUsers.delete(msg.data.user_id);
        this.updateSpeakingFlags();
      }
    }
  }

  updateSpeakingFlags() {
    let changed = false;
    for (const u of this.state.users) {
      const isSpeaking = this.speakingUsers.has(u.id);
      if (u.isSpeaking !== isSpeaking) {
        u.isSpeaking = isSpeaking;
        changed = true;
      }
    }
    if (changed) this.notifySubscribers();
  }

  updateVoiceChannel(data) {
    if (!data || !data.id) {
      this.state.inVoice = false;
      this.state.channelId = null;
      this.state.channelName = "";
      this.state.users = [];
      this.speakingUsers.clear();
      this.notifySubscribers();
      return;
    }

    this.state.inVoice = true;
    this.state.channelId = data.id;
    this.state.channelName = data.name || "Voice Channel";
    this.state.guildId = data.guild_id || null;

    if (data.id !== this.channelId) {
      this.channelId = data.id;
      this.subscribe("SPEAKING_START", { channel_id: data.id });
      this.subscribe("SPEAKING_STOP", { channel_id: data.id });
      this.subscribe("VOICE_STATE_CREATE", { channel_id: data.id });
      this.subscribe("VOICE_STATE_DELETE", { channel_id: data.id });
      this.subscribe("VOICE_STATE_UPDATE", { channel_id: data.id });
    }

    const voiceStates = data.voice_states || [];
    this.state.users = voiceStates.map((vs) => {
      const user = vs.user || {};
      const avatarUrl = user.avatar
        ? `https://cdn.discordapp.com/avatars/${user.id}/${user.avatar}.png?size=64`
        : null;

      // vol in discord is 0..200 (100 is 100%)
      const volumeLevel = typeof vs.volume === 'number' ? vs.volume / 100 : 1.0;

      return {
        id: user.id || Math.random().toString(),
        name: vs.nick || user.global_name || user.username || "User",
        avatar: avatarUrl,
        volume: Math.min(1.0, Math.max(0, volumeLevel)),
        isMuted: !!vs.mute,
        isSpeaking: this.speakingUsers.has(user.id)
      };
    });

    this.notifySubscribers();
  }

  // Action methods
  setMute(mute) {
    this.sendFrame("SET_VOICE_SETTINGS", { mute: !!mute });
  }

  setDeaf(deaf) {
    this.sendFrame("SET_VOICE_SETTINGS", { deaf: !!deaf });
  }

  setUserVolume(userId, volume) {
    // Discord takes 0..200 for volume percentage
    this.sendFrame("SET_USER_VOICE_SETTINGS", {
      user_id: userId,
      volume: Math.round(volume * 100)
    });
  }

  toggleUserMute(userId, mute) {
    this.sendFrame("SET_USER_VOICE_SETTINGS", {
      user_id: userId,
      mute: !!mute
    });
  }

  disconnect() {
    this.sendFrame("SELECT_VOICE_CHANNEL", { channel_id: null });
  }
}

let instance = null;
function getDiscordBridge() {
  if (!instance) {
    instance = new DiscordBridge();
  }
  return instance;
}

module.exports = { DiscordBridge, getDiscordBridge };
