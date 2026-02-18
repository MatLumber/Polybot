export function TechGrid() {
  return (
    <div className="fixed inset-0 -z-10 overflow-hidden pointer-events-none">
      {/* Animated gradient orbs */}
      <div className="absolute top-[-10%] left-[-5%] w-[600px] h-[600px] bg-primary/20 rounded-full mix-blend-screen filter blur-[100px] opacity-40 animate-blob" />
      <div className="absolute top-[-10%] right-[-5%] w-[600px] h-[600px] bg-profit/20 rounded-full mix-blend-screen filter blur-[100px] opacity-40 animate-blob animation-delay-2000" />
      <div className="absolute bottom-[-10%] left-[20%] w-[600px] h-[600px] bg-info/20 rounded-full mix-blend-screen filter blur-[100px] opacity-40 animate-blob animation-delay-4000" />
      
      {/* Grid pattern - más visible */}
      <svg className="absolute inset-0 w-full h-full opacity-30" xmlns="http://www.w3.org/2000/svg">
        <defs>
          <pattern id="tech-grid" width="50" height="50" patternUnits="userSpaceOnUse">
            <g className="text-primary/20" stroke="currentColor" strokeWidth="0.5" fill="none">
              <path d="M 50 0 L 0 0 0 50" />
            </g>
          </pattern>
          <pattern id="tech-dots" width="30" height="30" patternUnits="userSpaceOnUse">
            <circle cx="2" cy="2" r="1" className="text-primary/10" fill="currentColor" />
          </pattern>
          <linearGradient id="tech-gradient" x1="0%" y1="0%" x2="0%" y2="100%">
            <stop offset="0%" stopColor="transparent" stopOpacity="0" />
            <stop offset="50%" stopColor="hsl(var(--background))" stopOpacity="0.3" />
            <stop offset="100%" stopColor="hsl(var(--background))" stopOpacity="0.8" />
          </linearGradient>
          <radialGradient id="vignette">
            <stop offset="0%" stopColor="transparent" />
            <stop offset="70%" stopColor="transparent" />
            <stop offset="100%" stopColor="hsl(var(--background))" stopOpacity="0.8" />
          </radialGradient>
        </defs>
        <rect width="100%" height="100%" fill="url(#tech-grid)" />
        <rect width="100%" height="100%" fill="url(#tech-dots)" />
        <rect width="100%" height="100%" fill="url(#tech-gradient)" />
        <rect width="100%" height="100%" fill="url(#vignette)" />
      </svg>
      
      {/* Scanlines animadas - más visibles */}
      <div 
        className="absolute inset-0 opacity-[0.05]" 
        style={{
          backgroundImage: 'repeating-linear-gradient(0deg, transparent, transparent 2px, rgba(255,255,255,0.03) 2px, rgba(255,255,255,0.03) 4px)',
          animation: 'scanlines 8s linear infinite'
        }} 
      />
      
      {/* Noise texture */}
      <div 
        className="absolute inset-0 opacity-[0.02] mix-blend-overlay" 
        style={{
          backgroundImage: 'url("data:image/svg+xml,%3Csvg viewBox=\'0 0 200 200\' xmlns=\'http://www.w3.org/2000/svg\'%3E%3Cfilter id=\'noiseFilter\'%3E%3CfeTurbulence type=\'fractalNoise\' baseFrequency=\'0.9\' numOctaves=\'3\' stitchTiles=\'stitch\'/%3E%3C/filter%3E%3Crect width=\'100%\' height=\'100%\' filter=\'url(%23noiseFilter)\'/%3E%3C/svg%3E")'
        }} 
      />
    </div>
  );
}
