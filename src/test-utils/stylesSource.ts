// @ts-expect-error 项目未安装 @types/node；vitest 运行于 node，运行时可用
import { readFileSync } from "node:fs";

/**
 * 按 barrel 声明的顺序拼接 src/styles/ 分段文件，得到与应用实际下发一致的
 * 级联源码。CSS 校验类测试（时序同步、reduced-motion 覆盖等）应读这份
 * 而不是单个文件，避免拆分后漏检。
 */
export function readStylesSource(): string {
  // vitest 始终以仓库根目录为 CWD 运行
  const barrel: string = readFileSync("./src/styles.css", "utf8");
  return barrel
    .split("\n")
    .filter((line) => line.startsWith('@import "./styles/'))
    .map((line) => {
      const file = line.match(/@import "(.+?)";/)?.[1];
      if (!file) throw new Error(`无法解析 @import 行：${line}`);
      return readFileSync(`./src/${file}`, "utf8");
    })
    .join("\n");
}
