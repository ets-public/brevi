// A Worker that runs the Python an agent wrote in a Cloudflare Sandbox, compiled and held by Brevi.
//
//   POST /run   Authorization: Bearer <BREVI_TOKEN>   {"code": "...", "stdin": "...", "session": "task-42"}
//   -> {"exitCode": 0, "stdout": "...", "stderr": "..."}
//   DELETE /session/<name>  stops that session's container
//
// Each session name gets its own container (a Durable Object), kept warm for ten minutes between runs.
// What a program may do (ALLOW, ALLOW_NET, LIMITS in wrangler.jsonc) is set by whoever deploys the Worker;
// a request cannot widen it.
import { Files } from "@cloudflare/sandbox";
import { DurableObject } from "cloudflare:workers";

const WORKSPACE = "/workspace";
const INACTIVITY_TIMEOUT_MS = 10 * 60 * 1_000;
const SESSION = /^[a-z0-9][a-z0-9-]{0,62}$/;

interface Env {
  SANDBOX: DurableObjectNamespace<Sandbox>;
  BREVI_TOKEN: string;
  ALLOW: string;
  ALLOW_NET: string;
  LIMITS: string;
}

interface RunResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

// brevi run --sandbox [--allow ..] [--allow-net ..] [--limit ..] main.py, with stdin from stdin.txt.
function command(env: Env): string {
  const flags = ["--sandbox"];
  const clean = (s: string) => s.split(",").map((x) => x.trim()).filter((x) => /^[A-Za-z0-9.=_*-]+$/.test(x)).join(",");
  if (clean(env.ALLOW ?? "")) flags.push("--allow", clean(env.ALLOW));
  if (clean(env.ALLOW_NET ?? "")) flags.push("--allow-net", clean(env.ALLOW_NET));
  if (clean(env.LIMITS ?? "")) flags.push("--limit", clean(env.LIMITS));
  return `brevi run ${flags.join(" ")} main.py < stdin.txt`;
}

export class Sandbox extends DurableObject<Env> {
  async run(code: string, stdin: string): Promise<RunResult> {
    const container = await this.ensureRunning();
    const files = new Files(container);
    await files.writeFile(`${WORKSPACE}/main.py`, code);
    await files.writeFile(`${WORKSPACE}/stdin.txt`, stdin);
    const process = await container.exec(["sh", "-c", command(this.env)], { cwd: WORKSPACE });
    const out = await process.output();
    const text = new TextDecoder();
    return { exitCode: out.exitCode, stdout: text.decode(out.stdout), stderr: text.decode(out.stderr) };
  }

  async destroy(): Promise<void> {
    const container = this.ctx.container;
    if (container?.running) await container.destroy();
  }

  private async ensureRunning(): Promise<Container> {
    const container = this.ctx.container;
    if (container === undefined) throw new Error("the container binding is not configured");
    if (!container.running) {
      // The container reaches the internet only when programs are granted the network; Brevi then
      // limits them to the granted hosts.
      const net = Boolean(this.env.ALLOW_NET) || (this.env.ALLOW ?? "").split(",").includes("net");
      // rustc wants memory when it links a program; "lite" (256 MiB) is too small.
      container.start({ image: container.images.sandbox, instance: "standard-1", enableInternet: net });
      await container.setInactivityTimeout(INACTIVITY_TIMEOUT_MS);
    }
    return container;
  }
}

export default {
  async fetch(request, env): Promise<Response> {
    if (!env.BREVI_TOKEN || request.headers.get("Authorization") !== `Bearer ${env.BREVI_TOKEN}`) {
      return new Response("unauthorized", { status: 401 });
    }
    const url = new URL(request.url);
    if (url.pathname === "/run" && request.method === "POST") {
      let body: { code?: unknown; stdin?: unknown; session?: unknown };
      try {
        body = await request.json();
      } catch {
        return new Response('body must be JSON: {"code": "...", "stdin": "...", "session": "..."}', { status: 400 });
      }
      const session = typeof body.session === "string" ? body.session : "default";
      if (typeof body.code !== "string" || !SESSION.test(session)) {
        return new Response("code must be a string; session is lowercase letters, digits and dashes", { status: 400 });
      }
      const stdin = typeof body.stdin === "string" ? body.stdin : "";
      return Response.json(await env.SANDBOX.getByName(session).run(body.code, stdin));
    }
    const m = url.pathname.match(/^\/session\/([a-z0-9][a-z0-9-]{0,62})$/);
    if (m && request.method === "DELETE") {
      await env.SANDBOX.getByName(m[1]).destroy();
      return new Response(null, { status: 204 });
    }
    return new Response("not found", { status: 404 });
  },
} satisfies ExportedHandler<Env>;
