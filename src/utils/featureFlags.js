const flags = {
  NEW_LOBBY_SYSTEM: true,
  LIVE_CHAT: false,
  TOURNAMENTS: false,
  MOBILE_APP: false,
};

export function isEnabled(flag) {
  return flags[flag] === true;
}

export function getAllFlags() {
  return { ...flags };
}
