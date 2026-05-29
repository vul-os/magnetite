import Navbar from './Navbar';
import Footer from './Footer';

export default function LayoutComponent({ children }) {
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
