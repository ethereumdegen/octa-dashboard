import { Outlet } from "react-router";
import Sidebar from "./Sidebar";

export default function Layout() {
  return (
    <div className="min-h-screen bg-surface">
      <Sidebar />
      <main className="pb-20 px-4 pt-4 md:pb-8 md:ml-16 md:p-8">
        <Outlet />
      </main>
    </div>
  );
}
