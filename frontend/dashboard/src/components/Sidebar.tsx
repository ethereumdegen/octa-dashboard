import { Home, Bot, Users, Settings, Box, type LucideIcon } from "lucide-react";
import * as allIcons from "lucide-react";
import { useLocation, Link } from "react-router";
import { useAuth } from "../hooks/useAuth";
import { useMicroservices } from "../hooks/useMicroservices";

const coreNavItems = [
  { icon: Home, path: "/", label: "Home" },
  { icon: Bot, path: "/agents", label: "Agents" },
  { icon: Users, path: "/team", label: "Team" },
];

// Keep a reference to the full icon set so the bundler cannot tree-shake it.
const iconLookup: Record<string, unknown> = allIcons;

function resolveIcon(name: string): LucideIcon {
  const icon = iconLookup[name];
  if (typeof icon === "function" && "displayName" in icon) return icon as LucideIcon;
  return Box;
}

export default function Sidebar() {
  const location = useLocation();
  const { user } = useAuth();
  const { data: microservices } = useMicroservices();

  const microserviceNavItems = (microservices ?? [])
    .filter((ms) => ms.enabled)
    .map((ms) => ({
      icon: resolveIcon(ms.icon),
      path: ms.nav_path,
      label: ms.name,
    }));

  const navItems = [...coreNavItems, ...microserviceNavItems];
  const allNavItems = [...navItems, { icon: Settings, path: "/settings", label: "Settings" }];

  return (
    <>
      {/* Desktop sidebar */}
      <aside className="hidden md:flex fixed left-0 top-0 h-screen w-16 bg-white flex-col items-center py-6 shadow-sm z-50">
        <div className="mb-8">
          <div className="w-8 h-8 rounded-lg bg-gray-900 flex items-center justify-center text-white font-bold text-sm">
            T
          </div>
        </div>

        <nav className="flex flex-col items-center gap-2 flex-1">
          {navItems.map(({ icon: Icon, path, label }) => {
            const active = path === "/" ? location.pathname === "/" : location.pathname.startsWith(path);
            return (
              <Link
                key={path}
                to={path}
                title={label}
                className={`w-10 h-10 rounded-xl flex items-center justify-center transition-colors ${
                  active ? "bg-gray-100 text-gray-900" : "text-gray-400 hover:text-gray-600 hover:bg-gray-50"
                }`}
              >
                <Icon size={20} />
              </Link>
            );
          })}
        </nav>

        <div className="flex flex-col items-center gap-2">
          <Link
            to="/settings"
            title="Settings"
            className={`w-10 h-10 rounded-xl flex items-center justify-center transition-colors ${
              location.pathname === "/settings" ? "bg-gray-100 text-gray-900" : "text-gray-400 hover:text-gray-600 hover:bg-gray-50"
            }`}
          >
            <Settings size={20} />
          </Link>
          <div className="w-8 h-8 rounded-full bg-gray-200 flex items-center justify-center text-xs font-medium text-gray-600">
            {user?.email?.[0]?.toUpperCase() ?? "?"}
          </div>
        </div>
      </aside>

      {/* Mobile bottom nav */}
      <nav className="md:hidden fixed bottom-0 left-0 right-0 bg-white border-t border-gray-100 z-50 flex items-center justify-around px-2 py-1 safe-bottom">
        {allNavItems.map(({ icon: Icon, path, label }) => {
          const active = path === "/" ? location.pathname === "/" : location.pathname.startsWith(path);
          return (
            <Link
              key={path}
              to={path}
              className={`flex flex-col items-center gap-0.5 px-2 py-1.5 rounded-lg min-w-0 ${
                active ? "text-gray-900" : "text-gray-400"
              }`}
            >
              <Icon size={20} />
              <span className="text-[10px] font-medium truncate max-w-[4rem]">{label}</span>
            </Link>
          );
        })}
      </nav>
    </>
  );
}
