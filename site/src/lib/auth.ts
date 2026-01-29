import { betterAuth } from "better-auth";

export interface CloudflareEnv {
  DB: D1Database;
  BETTER_AUTH_KV: KVNamespace;
  BETTER_AUTH_SECRET: string;
  BETTER_AUTH_URL: string;
  GITHUB_CLIENT_ID?: string;
  GITHUB_CLIENT_SECRET?: string;
  GOOGLE_CLIENT_ID?: string;
  GOOGLE_CLIENT_SECRET?: string;
}

export function createAuth(env: CloudflareEnv) {
  return betterAuth({
    database: env.DB,
    secret: env.BETTER_AUTH_SECRET,
    baseURL: env.BETTER_AUTH_URL,
    secondaryStorage: {
      get: async (key: string) => {
        const value = await env.BETTER_AUTH_KV.get(key);
        return value ?? null;
      },
      set: async (key: string, value: string, ttl?: number) => {
        await env.BETTER_AUTH_KV.put(key, value, ttl ? { expirationTtl: ttl } : undefined);
      },
      delete: async (key: string) => {
        await env.BETTER_AUTH_KV.delete(key);
      },
    },
    emailAndPassword: {
      enabled: true,
    },
    socialProviders: {
      ...(env.GITHUB_CLIENT_ID && env.GITHUB_CLIENT_SECRET
        ? {
            github: {
              clientId: env.GITHUB_CLIENT_ID,
              clientSecret: env.GITHUB_CLIENT_SECRET,
            },
          }
        : {}),
      ...(env.GOOGLE_CLIENT_ID && env.GOOGLE_CLIENT_SECRET
        ? {
            google: {
              clientId: env.GOOGLE_CLIENT_ID,
              clientSecret: env.GOOGLE_CLIENT_SECRET,
            },
          }
        : {}),
    },
    trustedOrigins: [
      "http://localhost:3000",
      "https://pyro1121.com",
    ],
  });
}

export type Auth = ReturnType<typeof createAuth>;
