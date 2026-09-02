import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it } from "vitest";
import App from "@/App";

const pages = [
  ["Tools", "Tools"],
  ["Policies", "Policies"],
  ["Activity", "Activity"],
  ["Settings", "Settings"],
] as const;
const nav = (name: string) => screen.getByRole("button", { name: new RegExp(`${name}$`) });

afterEach(() => {
  cleanup();
  window.history.replaceState({}, "", "/");
});

describe("M1 application shell", () => {
  it("navigates across all five primary pages", async () => {
    render(<App />);
    expect(await screen.findByRole("heading", { name: "Overview", level: 1 })).toBeInTheDocument();
    for (const [navigationLabel, heading] of pages) {
      fireEvent.click(nav(navigationLabel));
      expect(await screen.findByRole("heading", { name: heading, level: 1 })).toBeInTheDocument();
    }
  });

  it("renders mock tools and policy decisions through the data provider", async () => {
    render(<App />);
    fireEvent.click(nav("Tools"));
    expect(await screen.findByText("read_file")).toBeInTheDocument();
    expect(screen.getByText("Not exposed")).toBeInTheDocument();
    fireEvent.click(nav("Policies"));
    expect(await screen.findByText("Allow workspace reads")).toBeInTheDocument();
    expect(screen.getAllByText("DENY").length).toBeGreaterThan(0);
  });

  it("renders the disconnected product state from the mock scenario", async () => {
    window.history.replaceState({}, "", "/?mock=disconnected");
    render(<App />);
    expect(await screen.findByText("Runtime not connected")).toBeInTheDocument();
  });

  it("renders empty and error states from the mock scenarios", async () => {
    window.history.replaceState({}, "", "/?mock=empty");
    render(<App />);
    fireEvent.click(nav("Tools"));
    expect(await screen.findByText("No tools are exposed in this mock scenario.")).toBeInTheDocument();
    cleanup();
    window.history.replaceState({}, "", "/?mock=error");
    render(<App />);
    expect(await screen.findByText("Couldn’t load this view")).toBeInTheDocument();
  });
});
