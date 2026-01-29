import { APIEvent } from "@solidjs/start/server";

export async function GET(event: APIEvent) {
  const cf = (event.nativeEvent as any).context?.cloudflare?.env;
  
  return new Response(JSON.stringify({
    hasDB: !!cf?.DB,
    hasKV: !!cf?.BETTER_AUTH_KV,
    hasSecret: !!cf?.BETTER_AUTH_SECRET,
    hasURL: !!cf?.BETTER_AUTH_URL,
    url: cf?.BETTER_AUTH_URL || "NOT SET",
    hasGithubId: !!cf?.GITHUB_CLIENT_ID,
    hasGithubSecret: !!cf?.GITHUB_CLIENT_SECRET,
    hasGoogleId: !!cf?.GOOGLE_CLIENT_ID,
    hasGoogleSecret: !!cf?.GOOGLE_CLIENT_SECRET,
    githubIdPrefix: cf?.GITHUB_CLIENT_ID?.substring(0, 4) || "NONE",
    googleIdPrefix: cf?.GOOGLE_CLIENT_ID?.substring(0, 4) || "NONE",
  }, null, 2), {
    headers: { "Content-Type": "application/json" }
  });
}
