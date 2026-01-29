import { betterAuth } from "better-auth";
import { drizzleAdapter } from "better-auth/adapters/drizzle";
import { drizzle } from "drizzle-orm/d1";
import * as schema from "../db/auth-schema";

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
  const db = drizzle(env.DB, { schema });
  
  return betterAuth({
    database: drizzleAdapter(db, {
      provider: "sqlite",
      usePlural: false,
    }),
    secret: env.BETTER_AUTH_SECRET,
    baseURL: env.BETTER_AUTH_URL,
    trustedOrigins: ["https://pyro1121.com"],
    secondaryStorage: {
      get: async (key: string) => {
        const value = await env.BETTER_AUTH_KV.get(key);
        return value ?? null;
      },
      set: async (key: string, value: string, ttl?: number) => {
        const minTtl = ttl && ttl < 60 ? 60 : ttl;
        await env.BETTER_AUTH_KV.put(key, value, minTtl ? { expirationTtl: minTtl } : undefined);
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
              redirectURI: "https://pyro1121.com/api/auth/callback/github",
            },
          }
        : {}),
      ...(env.GOOGLE_CLIENT_ID && env.GOOGLE_CLIENT_SECRET
        ? {
            google: {
              clientId: env.GOOGLE_CLIENT_ID,
              clientSecret: env.GOOGLE_CLIENT_SECRET,
              redirectURI: "https://pyro1121.com/api/auth/callback/google",
            },
          }
        : {}),
    },
  });
}

export type Auth = ReturnType<typeof createAuth>;
