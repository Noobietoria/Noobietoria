// End-to-end check: discover a server through the website API, then join it
// over the same newline-delimited JSON protocol the desktop client uses.
//
//   node scripts/e2e-join.mjs [apiBase]
import net from "node:net";

const apiBase = process.argv[2] || "http://127.0.0.1:3000";

async function main() {
    // 1. Ask the website which servers run the starter place.
    const res = await fetch(`${apiBase}/api/v1/places/starter_baseplate/servers`);
    if (!res.ok) throw new Error(`servers API returned ${res.status}`);
    const payload = await res.json();
    console.log(`API ${apiBase}/api/v1/places/starter_baseplate/servers -> count=${payload.count}`);
    if (!payload.count) throw new Error("no servers advertised");
    const server = payload.servers[0];
    console.log(
        `discovered: ${server.server_id} at ${server.host}:${server.port} ` +
        `(${server.region}, ${server.players}/${server.max_players}, ${server.status})`
    );

    // 2. Join it — no address typed by the player.
    await new Promise((resolve, reject) => {
        const sock = net.createConnection({ host: server.host, port: server.port }, () => {
            console.log(`connected to ${server.host}:${server.port}`);
            sock.write(JSON.stringify({ kind: "join", username: "E2EProbe", user_id: 4242, country: "VN" }) + "\n");
            sock.write(JSON.stringify({ kind: "ping" }) + "\n");
        });

        let buffer = "";
        const seen = { welcome: false, pong: false };
        const timer = setTimeout(() => {
            sock.destroy();
            reject(new Error(`timeout; saw ${JSON.stringify(seen)}`));
        }, 8000);

        sock.on("data", (chunk) => {
            buffer += chunk.toString();
            let idx;
            while ((idx = buffer.indexOf("\n")) >= 0) {
                const line = buffer.slice(0, idx).trim();
                buffer = buffer.slice(idx + 1);
                if (!line) continue;
                const msg = JSON.parse(line);
                console.log(`  <- ${JSON.stringify(msg)}`);
                if (msg.kind === "welcome") {
                    seen.welcome = true;
                    if (msg.server_name !== "Classic Baseplate #1") {
                        clearTimeout(timer);
                        sock.destroy();
                        reject(new Error(`unexpected server_name: ${msg.server_name}`));
                        return;
                    }
                }
                if (msg.kind === "pong") seen.pong = true;
                if (seen.welcome && seen.pong) {
                    clearTimeout(timer);
                    sock.end();
                    resolve();
                }
            }
        });

        sock.on("error", (e) => {
            clearTimeout(timer);
            reject(e);
        });
    });

    console.log("E2E OK: API-discovered endpoint completed join + ping/pong");
}

main().catch((e) => {
    console.error(`E2E FAILED: ${e.message}`);
    process.exit(1);
});
