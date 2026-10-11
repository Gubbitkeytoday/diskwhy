import axe from "axe-core";
import { expect } from "vitest";

/** Run axe-core against a container and fail with a readable list of violations. */
export async function expectNoA11yViolations(container: Element) {
  const result = await axe.run(container, {
    rules: {
      // jsdom has no layout engine, so colour contrast is verified at the token level instead (check-contrast.mjs).
      "color-contrast": { enabled: false },
      region: { enabled: false },
    },
  });
  const messages = result.violations.map(
    (v) => `${v.id} (${v.impact}): ${v.help}\n  ${v.nodes.map((n) => n.html).join("\n  ")}`,
  );
  expect(messages, messages.join("\n\n")).toEqual([]);
}
