const APP_VERSION_MAJOR = 1;
const APP_VERSION_MINOR = 0;

export const APP_NAME = "NSG Gaming Dub";
export const APP_VERSION = [APP_VERSION_MAJOR, APP_VERSION_MINOR].join(".");
export const APP_DISPLAY_NAME = `${APP_NAME} ${APP_VERSION}`;
export const ACTIVE_SPEECH_MODELS = [
  { label: "OmniVoice", value: "0.1.5" },
  { label: "Whisper", value: "large-v3" }
] as const;
