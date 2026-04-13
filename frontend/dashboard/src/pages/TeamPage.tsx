import { useState } from "react";
import { UserPlus, Trash2 } from "lucide-react";
import PageHeader from "../components/PageHeader";
import { useTeam } from "../hooks/useTeam";
import { useAuth } from "../hooks/useAuth";

function relativeTime(dateStr: string | null): string {
  if (!dateStr) return "Never";
  const now = Date.now();
  const then = new Date(dateStr).getTime();
  const days = Math.floor((now - then) / (1000 * 60 * 60 * 24));
  if (days === 0) return "Today";
  if (days === 1) return "1d";
  if (days <= 30) return `${days}d`;
  return ">30d";
}

export default function TeamPage() {
  const { data: members, isLoading, addMember, removeMember } = useTeam();
  const { isAdmin } = useAuth();
  const [email, setEmail] = useState("");
  const [role, setRole] = useState("member");

  const handleAdd = (e: React.FormEvent) => {
    e.preventDefault();
    if (!email.trim()) return;
    addMember.mutate({ email: email.trim(), role });
    setEmail("");
  };

  return (
    <div>
      <PageHeader title="Team" />

      {isAdmin && (
        <form onSubmit={handleAdd} className="bg-white rounded-2xl shadow-sm p-6 mb-6">
          <h3 className="text-sm font-medium text-gray-500 mb-4">Add Team Member</h3>
          <div className="flex flex-col sm:flex-row gap-3">
            <input
              type="email"
              value={email}
              onChange={(e) => setEmail(e.target.value)}
              placeholder="email@example.com"
              className="flex-1 bg-gray-100 rounded-xl px-4 py-2 text-sm outline-none focus:ring-2 focus:ring-gray-200"
            />
            <select
              value={role}
              onChange={(e) => setRole(e.target.value)}
              className="bg-gray-100 rounded-xl px-4 py-2 text-sm outline-none"
            >
              <option value="member">Member</option>
              <option value="admin">Admin</option>
            </select>
            <button
              type="submit"
              disabled={addMember.isPending}
              className="inline-flex items-center gap-2 bg-gray-900 text-white px-5 py-2 rounded-xl text-sm font-medium hover:bg-gray-800 transition-colors disabled:opacity-50"
            >
              <UserPlus size={16} />
              Add
            </button>
          </div>
        </form>
      )}

      <div className="bg-white rounded-2xl shadow-sm overflow-x-auto">
        <table className="w-full min-w-[600px]">
          <thead>
            <tr className="border-b border-gray-100">
              <th className="text-left text-xs font-medium text-gray-400 uppercase tracking-wider px-6 py-4">Email</th>
              <th className="text-left text-xs font-medium text-gray-400 uppercase tracking-wider px-6 py-4">Role</th>
              <th className="text-left text-xs font-medium text-gray-400 uppercase tracking-wider px-6 py-4">Added</th>
              <th className="text-left text-xs font-medium text-gray-400 uppercase tracking-wider px-6 py-4">Last Active</th>
              {isAdmin && <th className="w-12" />}
            </tr>
          </thead>
          <tbody>
            {isLoading && (
              <tr>
                <td colSpan={5} className="text-center py-8 text-sm text-gray-400">Loading...</td>
              </tr>
            )}
            {members?.map((member) => (
              <tr key={member.id} className="border-b border-gray-50 hover:bg-gray-50 transition-colors">
                <td className="px-6 py-4 text-sm text-gray-700">{member.email}</td>
                <td className="px-6 py-4">
                  <span className={`inline-flex items-center px-2.5 py-0.5 rounded-lg text-xs font-medium ${
                    member.role === "admin" ? "bg-gray-900 text-white" : "bg-gray-100 text-gray-600"
                  }`}>
                    {member.role}
                  </span>
                </td>
                <td className="px-6 py-4 text-sm text-gray-400">
                  {new Date(member.created_at).toLocaleDateString()}
                </td>
                <td className="px-6 py-4 text-sm text-gray-400">
                  {relativeTime(member.last_active)}
                </td>
                {isAdmin && (
                  <td className="px-6 py-4">
                    <button
                      onClick={() => removeMember.mutate(member.id)}
                      className="text-gray-400 hover:text-red-500 transition-colors"
                    >
                      <Trash2 size={14} />
                    </button>
                  </td>
                )}
              </tr>
            ))}
            {members?.length === 0 && (
              <tr>
                <td colSpan={5} className="text-center py-8 text-sm text-gray-400">No team members</td>
              </tr>
            )}
          </tbody>
        </table>
      </div>
    </div>
  );
}
