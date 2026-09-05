/**
 * Wardenly page bridge — injected via Page.addScriptToEvaluateOnNewDocument.
 *
 * Runs inside the game page and does two things:
 *
 *   1. Observe: patch Connection._parsePacket so every downstream protocol
 *      packet (already decoded by the game itself) is reported to the host
 *      through the `__wardenlyReport` CDP Runtime binding.
 *   2. Drive: expose `window.__wardenly.send(protocolName, payload)` which
 *      sends a protocol message through the game's own Connection (framing,
 *      encryption and encoding are all done by game code).
 *
 * The bridge never touches binary frames or cryptography, and every hook is
 * wrapped so a bridge failure can never break the game.
 *
 * See specs/proposals/protocol-driven-automation.md (Phase 2).
 */
(() => {
  // Init scripts run on every document of every navigation; install only once.
  if (window.__wardenly) return;

  const state = { ready: false };

  const report = (msg) => {
    try {
      if (typeof window.__wardenlyReport === 'function') {
        window.__wardenlyReport(JSON.stringify(msg));
      }
    } catch (e) {
      /* reporting must never break the game */
    }
  };

  window.__wardenly = {
    get ready() {
      return state.ready;
    },
    /**
     * Send a protocol message by name. Returns 'OK' or an 'ERR ...' string.
     * @param {string} name protocol name, e.g. 'C_2_S_MAIL_INFO'
     * @param {object} payload message fields per PROTOCOL_STRUCTS
     */
    send(name, payload) {
      try {
        const c = window.__require('Connection').default.get();
        const P = window.__require('ProtocolBase').Protocol;
        const id = P[name];
        if (typeof id !== 'number') return 'ERR unknown protocol: ' + name;
        const data = Object.assign({}, payload || {});
        // Auto-fill *_len companion fields from their string field, matching
        // the client's stringUTFLen semantics (UTF-8 byte length). Lets
        // templates pass raw strings without hard-coding lengths that break
        // on other servers (e.g. server_id differs across regions).
        for (const key of Object.keys(data)) {
          if (key.endsWith('_len')) {
            const base = key.slice(0, -4);
            if (typeof data[base] === 'string') {
              data[key] = new TextEncoder().encode(data[base]).length;
            }
          }
        }
        // Flag so the patched Connection.send can tag this as automation-originated.
        state.selfSend = true;
        c.send(id, data);
        return 'OK';
      } catch (e) {
        return 'ERR ' + (e && e.message ? e.message : String(e));
      }
    },    /**
     * Read a value from the game's own client role model (always current,
     * maintained by the game itself — no push required).
     * @param {string} path dotted path under role, e.g. '_militaryOrder' or
     *   '_knightTower._teamNumInfo.num'
     * @returns {string} JSON '{"ok":true,"value":...}' or 'ERR ...'
     */
    queryRole(path) {
      try {
        let v = window.__require('Account').default.get().role;
        for (const seg of String(path).split('.')) {
          if (seg === '') continue;
          if (v === null || v === undefined) return 'ERR unresolved: ' + path;
          v = v[seg];
        }
        if (v === undefined) return 'ERR unresolved: ' + path;
        return JSON.stringify({ ok: true, value: v });
      } catch (e) {
        return 'ERR ' + (e && e.message ? e.message : String(e));
      }
    },
  };

  const install = () => {
    try {
      // Only the game page has the module system; other pages simply never install.
      if (typeof window.__require !== 'function') return false;
      const c = window.__require('Connection').default.get();
      const P = window.__require('ProtocolBase').Protocol;
      if (!c || !P) return false;

      // Protocol enum is name -> id; invert it for id -> name lookup.
      const id2name = {};
      for (const key of Object.keys(P)) {
        const v = P[key];
        if (typeof v === 'number') id2name[v] = key;
      }

      if (!c.__wardenlyPatched) {
        const orig = c._parsePacket.bind(c);
        c._parsePacket = function (struct, buf) {
          const data = orig(struct, buf);
          try {
            // Protocol classes carry their id as a static `type`; nested plain
            // structs don't, which conveniently filters the recursive calls.
            const id = struct && struct.constructor && struct.constructor.type;
            if (typeof id === 'number' && data && typeof data === 'object') {
              // `pp` is the raw DataBuffer attached after parsing; strip it.
              const clean = Object.assign({}, data);
              delete clean.pp;
              report({ id: id, name: id2name[id] || null, data: clean });
            }
          } catch (e) {
            /* observe-only; never interfere */
          }
          return data;
        };
        c.__wardenlyPatched = true;
      }

      // Report client-originated upstream sends too: wrapping Connection.send
      // turns the page into a full traffic logger (kind='up'). Sends made
      // through __wardenly.send (automation) are flagged self=true so journals
      // can tell them apart from the game's own UI-driven traffic.
      if (!c.__wardenlySendPatched) {
        const origSend = c.send.bind(c);
        c.send = function (id, data) {
          try {
            const byAutomation = state.selfSend === true;
            state.selfSend = false;
            const clean = Object.assign({}, data || {});
            delete clean.pp;
            report({ dir: 'up', self: byAutomation, id: id, name: id2name[id] || null, data: clean });
          } catch (e) {
            /* observe-only; never interfere */
          }
          return origSend(id, data);
        };
        c.__wardenlySendPatched = true;
      }

      state.ready = true;
      return true;
    } catch (e) {
      return false;
    }
  };

  // The Connection singleton only exists after the game bundle boots; poll for it.
  const timer = setInterval(() => {
    if (install()) clearInterval(timer);
  }, 200);
  // Give up eventually so non-game pages don't poll forever.
  setTimeout(() => clearInterval(timer), 180000);
})();
