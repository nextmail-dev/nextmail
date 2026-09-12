import { describe, expect, it } from "vitest";

import { api } from "../api";
import { accountsApi } from "./accounts";
import { composerApi } from "./composer";
import { contactsApi } from "./contacts";
import { lifecycleApi } from "./lifecycle";
import { mailApi } from "./mail";
import { preferencesApi } from "./preferences";

describe("public API facade", () => {
  it("exposes every domain method exactly once", () => {
    const methodNames = [
      lifecycleApi,
      preferencesApi,
      accountsApi,
      mailApi,
      contactsApi,
      composerApi,
    ].flatMap(Object.keys);

    expect(new Set(methodNames).size).toBe(methodNames.length);
    expect(Object.keys(api).sort()).toEqual([...methodNames].sort());
  });
});
