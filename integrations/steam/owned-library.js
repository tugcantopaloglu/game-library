(async () => {
    // Credentials remain inside this dedicated Steam browser session. Only the
    // account ID and the validated game catalog are returned to Game Library.
    if (location.protocol !== "https:" || location.hostname !== "store.steampowered.com") return {kind:"waiting"};
    try {
        const element = document.getElementById("application_config");
        if (!element) return {kind:"waiting"};
        const user = JSON.parse(element.getAttribute("data-userinfo") || "null");
        if (!user?.logged_in || !/^\d{17}$/.test(String(user.steamid))) return {kind:"login_required"};
        const config = JSON.parse(element.getAttribute("data-store_user_config") || "null");
        if (typeof config?.webapi_token !== "string" || !config.webapi_token || config.webapi_token.length > 4096) return {kind:"unsupported"};
        const params = new URLSearchParams({access_token:config.webapi_token,input_json:JSON.stringify({steamid:user.steamid,include_appinfo:true,include_played_free_games:true,include_free_sub:true,skip_unvetted_apps:false})});
        const response = await fetch("https://api.steampowered.com/IPlayerService/GetOwnedGames/v1/?" + params, {credentials:"omit",cache:"no-store",redirect:"error",signal:AbortSignal.timeout(30000)});
        if (!response.ok) return {kind:"request_failed"};
        const length = Number(response.headers.get("content-length"));
        if (Number.isFinite(length) && length > 16 * 1024 * 1024) return {kind:"request_failed"};
        const reader = response.body?.getReader();
        if (!reader) return {kind:"request_failed"};
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
        if (!Number.isSafeInteger(value?.game_count) || value.game_count < 0 || value.game_count > 20000 || (value.games?.length || 0) !== value.game_count) return {kind:"request_failed"};
        const games = (value.games || []).map(row => ({appid:row.appid,name:row.name}));
        return {kind:"games",steamid:String(user.steamid),response:{game_count:value.game_count,games}};
    } catch { return {kind:"request_failed"}; }
})()
