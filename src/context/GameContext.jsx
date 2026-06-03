import { createContext, useContext, useState } from 'react';

const GameContext = createContext();

const MOCK_GAMES = [
  { id: 1, name: 'Lucky Slots', description: 'Spin to win big prizes', minBet: 1, maxBet: 100, category: 'slots' },
  { id: 2, name: 'Blackjack Pro', description: 'Beat the dealer', minBet: 5, maxBet: 500, category: 'cards' },
  { id: 3, name: 'Roulette Spin', description: 'Predict the winning number', minBet: 1, maxBet: 200, category: 'roulette' },
  { id: 4, name: 'Poker Texas', description: 'Texas Hold\'em poker', minBet: 10, maxBet: 1000, category: 'cards' },
  { id: 5, name: 'Dice Roll', description: 'Guess the outcome', minBet: 1, maxBet: 50, category: 'dice' },
  { id: 6, name: 'Baccarat Classic', description: 'Elegant card game', minBet: 10, maxBet: 500, category: 'cards' },
];

export function GameProvider({ children }) {
  const [games, setGames] = useState([]);
  const [currentGame, setCurrentGame] = useState(null);
  const [loading, setLoading] = useState(false);

  const fetchGames = async () => {
    setLoading(true);
    await new Promise(r => setTimeout(r, 300));
    setGames(MOCK_GAMES);
    setLoading(false);
  };

  const fetchGame = async (id) => {
    setLoading(true);
    await new Promise(r => setTimeout(r, 200));
    const game = MOCK_GAMES.find(g => g.id === parseInt(id));
    setCurrentGame(game || null);
    setLoading(false);
    return game || null;
  };

  return (
    <GameContext.Provider value={{ games, currentGame, loading, fetchGames, fetchGame }}>
      {children}
    </GameContext.Provider>
  );
}

// Provider + its consumer hook are intentionally colocated.
// eslint-disable-next-line react-refresh/only-export-components
export function useGames() {
  const context = useContext(GameContext);
  if (!context) throw new Error('useGames must be used within GameProvider');
  return context;
}
