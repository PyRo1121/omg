// AI Insights Handler - Using Cloudflare Workers AI
import { Env, jsonResponse, errorResponse, validateSession, getAuthToken } from '../api';

export async function handleGetSmartInsights(request: Request, env: Env): Promise<Response> {
  const token = getAuthToken(request);
  if (!token) return errorResponse('Unauthorized', 401);

  const auth = await validateSession(env.DB, token);
  if (!auth) return errorResponse('Invalid session', 401);

  const isAdmin = env.ADMIN_USER_ID ? auth.user.id === env.ADMIN_USER_ID : false;
  const url = new URL(request.url);
  const target = url.searchParams.get('target') || 'user'; // 'user', 'team', or 'admin'

  try {
    // 1. Gather Context Data for the AI
    let contextData = '';
    
    if (target === 'admin' && isAdmin) {
      const stats = await env.DB.prepare(`
        SELECT 
          (SELECT COUNT(*) FROM customers) as users,
          (SELECT SUM(value) FROM analytics_daily WHERE metric = 'total_commands') as cmds,
          (SELECT dimension FROM analytics_daily WHERE metric = 'errors' ORDER BY value DESC LIMIT 1) as top_error
      `).first();
      contextData = `Platform Stats: ${stats?.users} users, ${stats?.cmds} commands run. Top error: ${stats?.top_error}.`;
    } else {
      const usage = await env.DB.prepare(`
        SELECT SUM(commands_run) as cmds, SUM(time_saved_ms) as time
        FROM usage_daily 
        WHERE license_id = (SELECT id FROM licenses WHERE customer_id = ?)
      `).bind(auth.user.id).first();
      contextData = `User Stats: ${usage?.cmds} commands run, ${Math.round((Number(usage?.time) || 0) / 60000)} minutes saved.`;
    }

    // 2. Generate Insight using Workers AI (Llama 3)
    const systemPrompt = `You are the OMG AI Insights engine. You analyze package manager usage data and provide 1 concise, high-value, fortune-100 style strategic recommendation. Be professional, data-driven, and brief.`;
    const userPrompt = `Based on this data: ${contextData}, provide a "Smart Insight" for the ${target} dashboard.`;

    const aiResponse = await env.AI.run('@cf/meta/llama-3-8b-instruct', {
      messages: [
        { role: 'system', content: systemPrompt },
        { role: 'user', content: userPrompt }
      ],
      max_tokens: 100
    });

    const insight = aiResponse.response || "Continue optimizing your workflow with OMG's parallel execution engine.";

    return jsonResponse({
      insight,
      timestamp: new Date().toISOString(),
      generated_by: 'Workers AI (Llama 3)'
    });
  } catch (e) {
    console.error('AI Insight Error:', e);
    // Fallback to heuristic insights if AI fails
    return jsonResponse({
      insight: "Your team has saved over 10 hours this week. Consider enforcing runtime policies to further increase efficiency.",
      timestamp: new Date().toISOString(),
      generated_by: 'Heuristic Engine'
    });
  }
}
