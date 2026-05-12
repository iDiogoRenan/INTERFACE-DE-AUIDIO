import type { AppConfig } from "./types";

export type ConfigWithInputDirectory = AppConfig & { inputDir: string };

export function hasConfiguredInputDirectory(config: AppConfig): config is ConfigWithInputDirectory {
  return config.inputDir !== null && config.inputDir.trim().length > 0;
}
