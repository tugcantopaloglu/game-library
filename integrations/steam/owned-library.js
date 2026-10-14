(async () => {
    const validId = id => /^\d{17}$/.test(String(id));
    const catalog = (steamid, rows) => {
        if (!validId(steamid) || !Array.isArray(rows) || rows.length > 20000) return {kind:"request_failed"};
        const games = rows.map(row => ({appid:Number(row.appid),name:row.name}));
        if (games.some(row => !Number.isSafeInteger(row.appid) || row.appid <= 0 || typeof row.name !== "string" || !row.name.trim() || row.name.length > 512)) return {kind:"request_failed"};
        return {kind:"games",steamid:String(steamid),response:{game_count:games.length,games}};
    };
    if (location.protocol !== "https:") return {kind:"waiting"};
    if (location.hostname === "steamcommunity.com") {
        const id = location.pathname.match(/^\/profiles\/(\d{17})\/games\/?$/)?.[1];
        if (!id || String(globalThis.g_steamID) !== id) return {kind:"login_required"};
        let rows = globalThis.rgGames ?? globalThis.g_rgGames;
        if (!Array.isArray(rows)) {
            for (const script of document.scripts || []) {
                const match = script.textContent?.match(/(?:var\s+)?(?:rgGames|g_rgGames)\s*=\s*(\[[\s\S]*?\]);/);
                if (match && match[1].length <= 16 * 1024 * 1024) { try { rows = JSON.parse(match[1]); } catch {} break; }
            }
        }
        return rows ? catalog(id, rows) : {kind:"unsupported"};
    }
    if (location.hostname !== "store.steampowered.com") return {kind:"waiting"};
    let steamid;
    const fallback = () => {
        if (!validId(steamid) || typeof location.assign !== "function") return {kind:"request_failed"};
        location.assign("https://steamcommunity.com/profiles/" + steamid + "/games/?tab=all");
        return {kind:"navigating"};
    };
    try {
        const element = document.getElementById("application_config");
        if (!element) return {kind:"waiting"};
        const attribute = name => { try { return JSON.parse(element.getAttribute(name) || "null"); } catch { return null; } };
        const user = attribute("data-userinfo");
        if (!user?.logged_in || !validId(user.steamid)) return {kind:"login_required"};
        steamid = String(user.steamid);
        const config = attribute("data-store_user_config"), general = attribute("data-config");
        const token = config?.webapi_token ?? general?.webapi_token;
        if (typeof token !== "string" || !token || token.length > 4096) return fallback();
        const params = new URLSearchParams({access_token:token,input_json:JSON.stringify({steamid,include_appinfo:true,include_played_free_games:true,include_free_sub:true,skip_unvetted_apps:false})});
        const response = await fetch("https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/?" + params, {credentials:"omit",cache:"no-store",redirect:"error",signal:AbortSignal.timeout(30000)});
        if (!response.ok) return fallback();
        const length = Number(response.headers.get("content-length"));
        if (Number.isFinite(length) && length > 16 * 1024 * 1024) return {kind:"request_failed"};
        const reader = response.body?.getReader();
        if (!reader) return fallback();
        const decoder = new TextDecoder(); let text = "", size = 0;
        while (true) {
            const chunk = await reader.read();
            if (chunk.done) break;
            size += chunk.value.byteLength;
            if (size > 16 * 1024 * 1024) {await reader.cancel();return {kind:"request_failed"};}
            text += decoder.decode(chunk.value,{stream:true});
        }
        text += decoder.decode();
        const value = JSON.parse(text).response;
        if (!Number.isSafeInteger(value?.game_count) || value.game_count < 0 || value.game_count > 20000 || (value.games?.length || 0) !== value.game_count) return fallback();
        const result = catalog(steamid,value.games || []);
        return result.kind === "games" ? result : fallback();
    } catch { return fallback(); }
})()
