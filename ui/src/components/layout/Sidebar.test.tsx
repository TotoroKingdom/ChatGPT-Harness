import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { Sidebar } from "./Sidebar";

afterEach(cleanup);
it("keeps the frozen navigation order and sends the global Harness state to Overview", () => {
  const navigate = vi.fn();
  render(<Sidebar activePage="tools" onNavigate={navigate} adapterKind="real" runtimeStatus="ready" observed pendingCount={3} restartRequired />);
  const navigation = screen.getByRole("navigation");
  expect(Array.from(navigation.querySelectorAll("button")).map((button) => button.textContent)).toEqual(["Overview", "Tools", "Policies", "Approvals3", "Activity", "Settings"]);
  expect(screen.getByText("3")).toHaveAttribute("aria-label", "3 pending");
  fireEvent.click(screen.getByRole("button", { name: /Harness Ready/ }));
  expect(navigate).toHaveBeenCalledWith("overview");
  expect(screen.getByText("Restart required")).toBeInTheDocument();
});
it("does not manufacture Ready or a zero pending count for unread observations", () => {
  render(<Sidebar activePage="overview" onNavigate={() => undefined} adapterKind="real" runtimeStatus="ready" observed={false} />);
  expect(screen.getByRole("button", { name: "Harness Unknown" })).toBeInTheDocument();
  expect(screen.queryByText("0")).not.toBeInTheDocument();
  expect(screen.queryByText("Harness Ready")).not.toBeInTheDocument();
});
