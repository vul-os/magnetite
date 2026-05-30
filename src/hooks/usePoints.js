import { useState, useEffect, useCallback } from 'react';
import { api } from '../api/client';

// ── Mock data — only used when VITE_USE_MOCKS=true ──────────────────────────

const MOCK_BALANCE = {
  points: 4_820,
  lifetime_points: 32_400,
  rank: 142,
  season: {
    name: 'Season 3 — Iron Core',
    ends_at: '2026-08-31T23:59:59Z',
    tier: 'Gold',
    next_tier: 'Platinum',
    progress: 68,
    points_needed: 2_180,
  },
};

const MOCK_HISTORY = [
  { id: 1, type: 'earn',   amount: +500,  description: 'Win streak bonus — Cosmic Raiders', created_at: '2026-05-28T18:04:00Z' },
  { id: 2, type: 'earn',   amount: +250,  description: 'Daily login streak (7 days)',        created_at: '2026-05-27T09:00:00Z' },
  { id: 3, type: 'redeem', amount: -1000, description: 'Redeemed: Neon HUD Skin',            created_at: '2026-05-25T14:30:00Z' },
  { id: 4, type: 'earn',   amount: +750,  description: 'Tournament top-10 placement',        created_at: '2026-05-24T22:15:00Z' },
  { id: 5, type: 'earn',   amount: +100,  description: 'Achievement unlock: Speed Demon',    created_at: '2026-05-23T11:00:00Z' },
  { id: 6, type: 'earn',   amount: +200,  description: 'Referral bonus',                     created_at: '2026-05-20T08:45:00Z' },
];

const MOCK_REWARDS = [
  { id: 'r1', name: 'Neon HUD Skin',      description: 'Electric-teal HUD for any game.',  cost: 1_000, type: 'cosmetic', image: 'https://picsum.photos/seed/reward1/80/80', available: true },
  { id: 'r2', name: 'XP Boost (24h)',     description: '2× points for 24 hours.',           cost: 500,   type: 'boost',    image: 'https://picsum.photos/seed/reward2/80/80', available: true },
  { id: 'r3', name: 'Season Frame',       description: 'Exclusive Iron Core profile frame.', cost: 2_000, type: 'cosmetic', image: 'https://picsum.photos/seed/reward3/80/80', available: true },
  { id: 'r4', name: 'USDC Credit ($1)',   description: 'Convert 2,500 pts to $1 USDC.',     cost: 2_500, type: 'currency', image: 'https://picsum.photos/seed/reward4/80/80', available: true },
  { id: 'r5', name: 'Gold Badge',         description: 'Permanent Gold tier profile badge.', cost: 5_000, type: 'cosmetic', image: 'https://picsum.photos/seed/reward5/80/80', available: false },
];

const MOCK_LEADERBOARD = [
  { rank: 1,  username: 'VoidStriker',   points: 98_450, avatar: 'https://picsum.photos/seed/void/40/40' },
  { rank: 2,  username: 'NeonHunter',    points: 87_220, avatar: 'https://picsum.photos/seed/neon/40/40' },
  { rank: 3,  username: 'IronCorePilot', points: 74_100, avatar: 'https://picsum.photos/seed/iron/40/40' },
  { rank: 4,  username: 'PixelStorm',    points: 62_800, avatar: 'https://picsum.photos/seed/pixel/40/40' },
  { rank: 5,  username: 'RustBorn',      points: 55_300, avatar: 'https://picsum.photos/seed/rust/40/40' },
  { rank: 6,  username: 'StellarDrift',  points: 48_700, avatar: 'https://picsum.photos/seed/stellar/40/40' },
  { rank: 7,  username: 'MagCore',       points: 41_200, avatar: 'https://picsum.photos/seed/magcore/40/40' },
  { rank: 8,  username: 'FerroBlast',    points: 37_900, avatar: 'https://picsum.photos/seed/ferro/40/40' },
  { rank: 9,  username: 'OxideRacer',    points: 32_500, avatar: 'https://picsum.photos/seed/oxide/40/40' },
  { rank: 10, username: 'CrystalByte',   points: 29_100, avatar: 'https://picsum.photos/seed/crystal/40/40' },
];

const USE_MOCKS = import.meta.env.VITE_USE_MOCKS === 'true';

// ─────────────────────────────────────────────────────────────────────────────

export function usePoints() {
  const [balance, setBalance]         = useState(USE_MOCKS ? MOCK_BALANCE : null);
  const [history, setHistory]         = useState(USE_MOCKS ? MOCK_HISTORY : []);
  const [rewards, setRewards]         = useState(USE_MOCKS ? MOCK_REWARDS : []);
  const [leaderboard, setLeaderboard] = useState(USE_MOCKS ? MOCK_LEADERBOARD : []);
  const [loading, setLoading]         = useState(!USE_MOCKS);
  const [error, setError]             = useState(null);
  const [redeeming, setRedeeming]     = useState(false);

  useEffect(() => {
    if (USE_MOCKS) return;

    let cancelled = false;

    async function load() {
      setLoading(true);
      setError(null);
      try {
        const [balRes, histRes, rewRes, lbRes] = await Promise.allSettled([
          api.points.balance(),
          api.points.history({ limit: 20 }),
          api.points.rewards(),
          api.points.leaderboard({ limit: 10 }),
        ]);

        if (!cancelled) {
          if (balRes.status === 'fulfilled' && balRes.value) {
            setBalance(balRes.value);
          } else if (balRes.status === 'rejected') {
            setError(balRes.reason?.message ?? 'Failed to load points balance');
          }

          if (histRes.status === 'fulfilled' && Array.isArray(histRes.value?.history)) {
            setHistory(histRes.value.history);
          } else if (histRes.status === 'fulfilled' && Array.isArray(histRes.value)) {
            setHistory(histRes.value);
          }

          if (rewRes.status === 'fulfilled' && Array.isArray(rewRes.value?.rewards)) {
            setRewards(rewRes.value.rewards);
          } else if (rewRes.status === 'fulfilled' && Array.isArray(rewRes.value)) {
            setRewards(rewRes.value);
          }

          if (lbRes.status === 'fulfilled' && Array.isArray(lbRes.value?.entries)) {
            setLeaderboard(lbRes.value.entries);
          } else if (lbRes.status === 'fulfilled' && Array.isArray(lbRes.value)) {
            setLeaderboard(lbRes.value);
          }
        }
      } finally {
        if (!cancelled) setLoading(false);
      }
    }

    load();
    return () => { cancelled = true; };
  }, []);

  const redeem = useCallback(async (rewardId) => {
    setRedeeming(true);
    try {
      const result = await api.points.redeem({ reward_id: rewardId });
      const reward = rewards.find(r => r.id === rewardId);
      if (result?.points != null) {
        setBalance(b => ({ ...b, points: result.points }));
      } else if (reward) {
        setBalance(b => b ? { ...b, points: Math.max(0, b.points - reward.cost) } : b);
      }
      setHistory(h => [
        { id: Date.now(), type: 'redeem', amount: -(reward?.cost ?? 0), description: `Redeemed: ${reward?.name ?? 'Reward'}`, created_at: new Date().toISOString() },
        ...h,
      ]);
      return { success: true };
    } catch (err) {
      return { success: false, error: err.message };
    } finally {
      setRedeeming(false);
    }
  }, [rewards]);

  return { balance, history, rewards, leaderboard, loading, error, redeeming, redeem };
}
