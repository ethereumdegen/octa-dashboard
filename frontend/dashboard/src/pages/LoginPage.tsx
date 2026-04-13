import { Github } from "lucide-react";

export default function LoginPage() {
  const githubClientId = import.meta.env.VITE_GITHUB_CLIENT_ID || "";
  const redirectUri = import.meta.env.VITE_GITHUB_REDIRECT_URI || "http://localhost:8080/auth/callback";

  const handleLogin = () => {
    const state = crypto.randomUUID();
    document.cookie = `oauth_state=${state}; path=/; max-age=600; samesite=lax`;
    window.location.href = `https://github.com/login/oauth/authorize?client_id=${githubClientId}&redirect_uri=${encodeURIComponent(redirectUri)}&scope=user:email&state=${state}`;
  };

  return (
    <div className="min-h-screen bg-surface flex items-center justify-center">
      <div className="bg-white rounded-2xl shadow-sm p-8 w-full max-w-sm text-center">
        <div className="w-12 h-12 rounded-xl bg-gray-900 flex items-center justify-center text-white font-bold text-lg mx-auto mb-6">
          O
        </div>
        <h1 className="text-xl font-bold text-gray-900 mb-2">Welcome to Octa</h1>
        <p className="text-sm text-gray-400 mb-8">Sign in to access the dashboard</p>
        <button
          onClick={handleLogin}
          className="inline-flex items-center gap-2 bg-gray-900 text-white px-6 py-3 rounded-xl font-medium text-sm hover:bg-gray-800 transition-colors w-full justify-center"
        >
          <Github size={18} />
          Sign in with GitHub
        </button>
      </div>
    </div>
  );
}
