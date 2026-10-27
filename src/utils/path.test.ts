import { describe, expect, it } from "vitest";
import { parentPath } from "./path";

describe("parentPath", () => {
  it("handles Windows drive roots", () => {
    expect(parentPath("C:\\")).toBe("C:\\");
    expect(parentPath("C:/")).toBe("C:\\");
    expect(parentPath("C:\\Users")).toBe("C:\\");
    expect(parentPath("C:\\Users\\me")).toBe("C:\\Users");
    expect(parentPath("D:/work/project/")).toBe("D:/work");
    expect(parentPath("D:")).toBe("D:\\");
  });

  it("handles POSIX paths", () => {
    expect(parentPath("/")).toBe("/");
    expect(parentPath("/home")).toBe("/");
    expect(parentPath("/home/me/project")).toBe("/home/me");
    expect(parentPath("a/b")).toBe("a");
    expect(parentPath("a")).toBe(".");
  });

  it("handles UNC paths", () => {
    expect(parentPath("\\\\server\\share")).toBe("\\\\server\\share");
    expect(parentPath("\\\\server\\share\\dir")).toBe("\\\\server\\share");
    expect(parentPath("\\\\server\\share\\dir\\sub")).toBe("\\\\server\\share\\dir");
  });

  it("handles empty input", () => {
    expect(parentPath("")).toBe("/");
    expect(parentPath("   ")).toBe("/");
  });
});
