export const mockProfileUser = {
  id: '1',
  username: 'SpeedDemon',
  email: 'speeddemon@example.com',
  bio: 'Professional racer | 3x Champion | Streaming daily',
  location: 'Los Angeles, CA',
  avatar: 'https://picsum.photos/seed/speeddemon/200/200',
  coverImage: 'https://picsum.photos/seed/cover1/1200/300',
  stats: {
    gamesPlayed: 1247,
    achievements: 42,
    friends: 156,
  },
  joinedAt: '2024-01-15T00:00:00Z',
  isOnline: true,
};

export const mockRecentGames = [
  { id: '1', title: 'Cosmic Drift', thumbnail: 'https://picsum.photos/seed/cosmic/400/300', playedAt: '2026-05-18T14:30:00Z', score: 9850, rank: 1 },
  { id: '2', title: 'Neon Striker', thumbnail: 'https://picsum.photos/seed/neon/400/300', playedAt: '2026-05-17T18:00:00Z', score: 7234, rank: 3 },
  { id: '3', title: 'Quantum Heist', thumbnail: 'https://picsum.photos/seed/quantum/400/300', playedAt: '2026-05-16T20:15:00Z', score: 4500, rank: 2 },
];

export const mockProfileAchievements = [
  { id: '1', name: 'First Blood', icon: '⚔️', unlockedAt: '2026-01-15T10:00:00Z' },
  { id: '3', name: 'Social Butterfly', icon: '🦋', unlockedAt: '2026-02-20T14:30:00Z' },
  { id: '5', name: 'High Roller', icon: '🎰', unlockedAt: '2026-03-01T09:00:00Z' },
];

export const mockProfileFriends = [
  { id: '2', username: 'CosmicKing', status: 'offline', avatar: 'https://picsum.photos/seed/user2/100/100' },
  { id: '3', username: 'DriftMaster', status: 'online', avatar: 'https://picsum.photos/seed/user3/100/100' },
  { id: '4', username: 'NebulaRacer', status: 'offline', avatar: 'https://picsum.photos/seed/user4/100/100' },
  { id: '8', username: 'NeonChampion', status: 'online', avatar: 'https://picsum.photos/seed/user8/100/100' },
];
