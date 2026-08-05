import type { ReactNode } from 'react';
import Navbar from './Navbar';
import Footer from './Footer';

interface LayoutComponentProps {
  children?: ReactNode;
}

export default function LayoutComponent({ children }: LayoutComponentProps) {
  return (
    <div className="app-layout">
      <Navbar />
      <main id="main-content" className="main-content">
        {children}
      </main>
      <Footer />
    </div>
  );
}

export { Navbar, Footer };
