import { createSignal, Show } from "solid-js";
import { A, useNavigate } from "@solidjs/router";
import { Title, Meta } from "@solidjs/meta";
import { signUp, signIn, useSession } from "~/lib/auth-client";
import { Terminal, Github, Mail, User, AlertCircle, Loader2 } from "lucide-solid";

export default function SignupPage() {
  const navigate = useNavigate();
  const session = useSession();
  const [name, setName] = createSignal("");
  const [email, setEmail] = createSignal("");
  const [password, setPassword] = createSignal("");
  const [confirmPassword, setConfirmPassword] = createSignal("");
  const [loading, setLoading] = createSignal(false);
  const [error, setError] = createSignal("");

  if (session()?.user) {
    navigate("/dashboard", { replace: true });
  }

  const handleSignup = async (e: Event) => {
    e.preventDefault();
    setLoading(true);
    setError("");

    if (password() !== confirmPassword()) {
      setError("Passwords do not match");
      setLoading(false);
      return;
    }

    if (password().length < 8) {
      setError("Password must be at least 8 characters");
      setLoading(false);
      return;
    }

    try {
      const result = await signUp.email({
        email: email(),
        password: password(),
        name: name(),
      });

      if (result.error) {
        setError(result.error.message || "Signup failed");
      } else {
        // Auto-provision license for new user
        try {
          await fetch('/api/provision-license', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
          });
        } catch (err) {
          console.warn('License auto-provision failed (non-critical):', err);
        }
        navigate("/dashboard");
      }
    } catch (err) {
      setError((err as Error).message || "An unexpected error occurred");
    } finally {
      setLoading(false);
    }
  };

  const handleOAuthLogin = async (provider: "github" | "google") => {
    setLoading(true);
    setError("");

    try {
      await signIn.social({
        provider,
        callbackURL: "/dashboard",
      });
    } catch (err) {
      setError((err as Error).message || "OAuth signup failed");
      setLoading(false);
    }
  };

  const pageBg =
    "min-h-screen bg-[#0a0a0a] text-slate-200 font-sans selection:bg-blue-500/30 selection:text-blue-200 overflow-x-hidden relative";
  const glassPanel =
    "bg-white/5 backdrop-blur-xl border border-white/10 rounded-2xl shadow-2xl";
  const glassInput =
    "w-full bg-white/5 border border-white/10 rounded-xl px-4 py-3 text-white placeholder-white/30 focus:outline-none focus:border-blue-500/50 focus:ring-1 focus:ring-blue-500/50 transition-all";
  const glassButton =
    "w-full bg-gradient-to-r from-blue-600 to-indigo-600 hover:from-blue-500 hover:to-indigo-500 text-white font-medium py-3 rounded-xl shadow-lg shadow-blue-500/20 transition-all active:scale-[0.98] disabled:opacity-50 disabled:cursor-not-allowed";
  const oauthButton =
    "w-full flex items-center justify-center gap-3 bg-white/5 border border-white/10 hover:bg-white/10 text-white font-medium py-3 rounded-xl transition-all";

  return (
    <>
      <Title>Sign Up - OMG Package Manager</Title>
      <Meta name="description" content="Create your OMG Package Manager account" />
      <Meta name="robots" content="noindex, nofollow" />

      <div class={pageBg}>
        <div class="pointer-events-none fixed top-[-20%] left-[-10%] h-[50%] w-[50%] rounded-full bg-purple-600/10 blur-[120px]" />
        <div class="pointer-events-none fixed right-[-10%] bottom-[-20%] h-[50%] w-[50%] rounded-full bg-blue-600/10 blur-[120px]" />
        <div class="pointer-events-none fixed top-[30%] right-[5%] h-[25%] w-[25%] rounded-full bg-emerald-600/5 blur-[100px]" />

        <div class="relative z-10 flex min-h-screen flex-col items-center justify-center p-4">
          <div class={`${glassPanel} animate-fade-in w-full max-w-md p-8 md:p-12`}>
            <div class="mb-8 text-center">
              <div class="mb-6 inline-flex h-16 w-16 items-center justify-center rounded-2xl bg-gradient-to-tr from-purple-500 to-blue-500 shadow-lg shadow-purple-500/20">
                <Terminal class="h-8 w-8 text-white" />
              </div>
              <h1 class="mb-2 text-3xl font-bold tracking-tight text-white">Create account</h1>
              <p class="text-slate-400">Get started with OMG Package Manager</p>
            </div>

            <div class="mb-6 space-y-3">
              <button
                type="button"
                onClick={() => handleOAuthLogin("github")}
                disabled={loading()}
                class={oauthButton}
              >
                <Github class="h-5 w-5" />
                Sign up with GitHub
              </button>
              <button
                type="button"
                onClick={() => handleOAuthLogin("google")}
                disabled={loading()}
                class={oauthButton}
              >
                <svg class="h-5 w-5" viewBox="0 0 24 24">
                  <path
                    fill="currentColor"
                    d="M22.56 12.25c0-.78-.07-1.53-.2-2.25H12v4.26h5.92c-.26 1.37-1.04 2.53-2.21 3.31v2.77h3.57c2.08-1.92 3.28-4.74 3.28-8.09z"
                  />
                  <path
                    fill="currentColor"
                    d="M12 23c2.97 0 5.46-.98 7.28-2.66l-3.57-2.77c-.98.66-2.23 1.06-3.71 1.06-2.86 0-5.29-1.93-6.16-4.53H2.18v2.84C3.99 20.53 7.7 23 12 23z"
                  />
                  <path
                    fill="currentColor"
                    d="M5.84 14.09c-.22-.66-.35-1.36-.35-2.09s.13-1.43.35-2.09V7.07H2.18C1.43 8.55 1 10.22 1 12s.43 3.45 1.18 4.93l2.85-2.22.81-.62z"
                  />
                  <path
                    fill="currentColor"
                    d="M12 5.38c1.62 0 3.06.56 4.21 1.64l3.15-3.15C17.45 2.09 14.97 1 12 1 7.7 1 3.99 3.47 2.18 7.07l3.66 2.84c.87-2.6 3.3-4.53 6.16-4.53z"
                  />
                </svg>
                Sign up with Google
              </button>
            </div>

            <div class="relative mb-6">
              <div class="absolute inset-0 flex items-center">
                <div class="w-full border-t border-white/10" />
              </div>
              <div class="relative flex justify-center text-sm">
                <span class="bg-[#0a0a0a] px-4 text-slate-500">or create with email</span>
              </div>
            </div>

            <form onSubmit={handleSignup} class="space-y-4">
              <div>
                <label class="mb-2 ml-1 block text-sm font-medium text-slate-300">
                  Full Name
                </label>
                <div class="relative">
                  <User class="absolute left-4 top-1/2 h-5 w-5 -translate-y-1/2 text-slate-500" />
                  <input
                    type="text"
                    value={name()}
                    onInput={(e) => setName(e.currentTarget.value)}
                    placeholder="John Doe"
                    required
                    class={`${glassInput} pl-12`}
                  />
                </div>
              </div>

              <div>
                <label class="mb-2 ml-1 block text-sm font-medium text-slate-300">
                  Email Address
                </label>
                <div class="relative">
                  <Mail class="absolute left-4 top-1/2 h-5 w-5 -translate-y-1/2 text-slate-500" />
                  <input
                    type="email"
                    value={email()}
                    onInput={(e) => setEmail(e.currentTarget.value)}
                    placeholder="dev@example.com"
                    required
                    class={`${glassInput} pl-12`}
                  />
                </div>
              </div>

              <div>
                <label class="mb-2 ml-1 block text-sm font-medium text-slate-300">
                  Password
                </label>
                <input
                  type="password"
                  value={password()}
                  onInput={(e) => setPassword(e.currentTarget.value)}
                  placeholder="At least 8 characters"
                  required
                  minLength={8}
                  class={glassInput}
                />
              </div>

              <div>
                <label class="mb-2 ml-1 block text-sm font-medium text-slate-300">
                  Confirm Password
                </label>
                <input
                  type="password"
                  value={confirmPassword()}
                  onInput={(e) => setConfirmPassword(e.currentTarget.value)}
                  placeholder="Confirm your password"
                  required
                  class={glassInput}
                />
              </div>

              <Show when={error()}>
                <div class="flex items-center gap-2 rounded-lg border border-red-500/20 bg-red-500/10 p-3 text-sm text-red-400">
                  <AlertCircle class="h-4 w-4 flex-shrink-0" />
                  {error()}
                </div>
              </Show>

              <button type="submit" disabled={loading()} class={glassButton}>
                {loading() ? (
                  <span class="flex items-center justify-center gap-2">
                    <Loader2 class="h-4 w-4 animate-spin" />
                    Creating account...
                  </span>
                ) : (
                  "Create Account"
                )}
              </button>
            </form>

            <div class="mt-6 text-center">
              <p class="text-sm text-slate-400">
                Already have an account?{" "}
                <A href="/login" class="font-medium text-blue-400 hover:text-blue-300">
                  Sign in
                </A>
              </p>
            </div>
          </div>

          <A
            href="/"
            class="mt-8 text-sm text-slate-500 transition-colors hover:text-white"
          >
            ← Back to home
          </A>
        </div>
      </div>
    </>
  );
}
