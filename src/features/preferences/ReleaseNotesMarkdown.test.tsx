import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";

import { ReleaseNotesMarkdown } from "./ReleaseNotesMarkdown";

describe("ReleaseNotesMarkdown", () => {
  it("renders Markdown without raw HTML, images, or unsafe links", () => {
    const { container } = render(
      <ReleaseNotesMarkdown>{[
        "## Fixes",
        "",
        "- Render **lists** and `code`.",
        "- [Safe](https://example.com/release)",
        "- [Unsafe](javascript:alert(1))",
        "- ![Tracking](https://example.com/pixel.png)",
        "<script>window.evil = true</script>",
      ].join("\n")}</ReleaseNotesMarkdown>,
    );

    expect(screen.getByRole("heading", { name: "Fixes" })).toBeInTheDocument();
    expect(screen.getByText("lists").tagName).toBe("STRONG");
    expect(screen.getByText("code").tagName).toBe("CODE");
    expect(screen.getByRole("link", { name: "Safe" })).toHaveAttribute("target", "_blank");
    expect(screen.queryByRole("link", { name: "Unsafe" })).not.toBeInTheDocument();
    expect(container.querySelector("script")).not.toBeInTheDocument();
    expect(container.querySelector("img")).not.toBeInTheDocument();
  });
});
