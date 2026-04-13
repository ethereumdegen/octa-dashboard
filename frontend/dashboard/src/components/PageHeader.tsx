import { Search } from "lucide-react";
import { useAuth } from "../hooks/useAuth";

interface PageHeaderProps {
  title: string;
}

export default function PageHeader({ title }: PageHeaderProps) {
  const { user } = useAuth();

  return (
    <div className="flex items-center justify-between mb-6 md:mb-8">
      <h1 className="text-xl md:text-2xl font-bold text-gray-900">{title}</h1>
      <div className="flex items-center gap-4">
        <div className="relative hidden sm:block">
          <Search size={16} className="absolute left-3 top-1/2 -translate-y-1/2 text-gray-400" />
          <input
            type="text"
            placeholder="Search..."
            className="pl-9 pr-4 py-2 bg-gray-100 rounded-xl text-sm text-gray-600 placeholder-gray-400 outline-none focus:ring-2 focus:ring-gray-200 w-64"
          />
        </div>
        <div className="w-8 h-8 rounded-full bg-gray-200 flex items-center justify-center text-xs font-medium text-gray-600 hidden md:flex">
          {user?.email?.[0]?.toUpperCase() ?? "?"}
        </div>
      </div>
    </div>
  );
}
