const flags = {
  NEW_LOBBY_SYSTEM: true,
  LIVE_CHAT: false,
  TOURNAMENTS: false,
  MOBILE_APP: false,
};

export type FeatureFlag = keyof typeof flags;

export function isEnabled(flag: FeatureFlag): boolean {
  return flags[flag] === true;
}

export function getAllFlags(): typeof flags {
  return { ...flags };
}
